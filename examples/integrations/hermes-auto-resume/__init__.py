"""Hermes plugin: bind a cflx enqueue to the thread that asked for it.

# The problem it solves

MCP does not tell a server which Hermes turn called it, and asking the model to
construct a callback command is prompt-compliance, not engineering. A
``post_tool_call`` hook is the one place that sees both halves: the tool result
(which carries the execution binding) and the request-scoped messaging context
the call was made in. So the plugin — not the model — registers the completion
sink, while the return address is still known.

# What it does not do

It does not rewrite tool results, inspect unrelated tools, parse shell commands,
or interpret the model's text. It filters one exact tool name, reads three
fields out of a versioned envelope, three more out of the session context and
one connection argument out of the call itself, and makes one bounded
subprocess call.

Everything it reads is scoped to the call in front of it. The owner a
registration is sent to comes from that call's own ``project_dir`` argument —
or its low-level ``unix_socket`` when that was the selector — so one Hermes
process can serve several Conflux projects without keeping, or being able to
stale, a project-to-socket map.

It calls ``cflx client notify set`` directly rather than re-entering the MCP
tool from inside a hook: that keeps the hook independent of the model and the
tool dispatcher, while using the same Unix-socket owner boundary the MCP tool
uses. Sink registration is accepted only over that socket, because it stores an
argv the owner will execute.
"""

from __future__ import annotations

import json
import logging
import os
import shutil
import subprocess
import sys
from typing import Any, Dict, Optional

from .cflx_hermes_resume import (
    CALL_PROJECT_ARGUMENT,
    CALL_SOCKET_ARGUMENT,
    CALLBACK_ENVIRONMENT_KEYS,
    DEFAULT_SEARCH_PATH,
    PROJECT_ROUTE,
    ROUTE_OPTIONS,
    SOCKET_ROUTE,
    build_target,
    delivery_environment,
    extract_binding,
    is_enqueue_tool,
    require_executable,
    require_messaging_surface,
    resolve_owner_route,
)

logger = logging.getLogger(__name__)

#: Ceiling on the registration subprocess. Registration is a local Unix-socket
#: round trip; anything longer than this is a stuck owner, not a slow one.
REGISTRATION_TIMEOUT_SECONDS = 20.0

#: The one outcome that means a sink is attached.
SUBSCRIBED = "subscribed"

#: Absolute path of the callback this plugin registers. Absolute because the
#: owner replaces the callback's environment: there is no ``PATH`` to resolve a
#: name against, and no interpreter on it to find.
CALLBACK_PATH = os.path.join(os.path.dirname(os.path.abspath(__file__)), "cflx_hermes_resume.py")

#: The request-scoped bindings that name where a reply belongs.
SESSION_PLATFORM = "HERMES_SESSION_PLATFORM"
SESSION_SOURCE = "HERMES_SESSION_SOURCE"
SESSION_CHAT_ID = "HERMES_SESSION_CHAT_ID"
SESSION_THREAD_ID = "HERMES_SESSION_THREAD_ID"

SESSION_BINDINGS = (SESSION_PLATFORM, SESSION_SOURCE, SESSION_CHAT_ID, SESSION_THREAD_ID)


def read_session_bindings() -> Dict[str, str]:
    """Where this turn came from, read the way concurrency requires.

    The gateway serves messages concurrently and binds these per asyncio task
    through ``contextvars``; ``os.environ`` is a process-global mirror written
    last-writer-wins by whichever turn ran most recently. Reading the mirror
    from inside a hook is therefore how a reply ends up in a stranger's chat.

    So: the context is authoritative wherever it exists, and the mirror is read
    only when this process has never bound a session at all — a bare CLI, a cron
    invocation, or a test — where there is no second turn to confuse it with.
    """
    try:
        from gateway.session_context import (  # type: ignore[import-not-found]
            get_session_env,
            session_context_engaged,
        )
    except Exception:  # noqa: BLE001 - a Hermes without this module is not an error
        return {name: os.environ.get(name, "") for name in SESSION_BINDINGS}

    try:
        engaged = bool(session_context_engaged())
    except Exception:  # noqa: BLE001
        engaged = True

    bindings = {}
    for name in SESSION_BINDINGS:
        try:
            value = get_session_env(name, "") or ""
        except Exception:  # noqa: BLE001
            value = ""
        if not value and not engaged:
            value = os.environ.get(name, "")
        bindings[name] = value
    return bindings


def resolve_target(bindings: Dict[str, str]) -> str:
    """The ``hermes send --to`` destination for this turn, or refuse to have one.

    Raises ``ValueError`` when the turn has no messaging return address, which
    is a reason to register nothing rather than a reason to guess one.
    """
    require_messaging_surface(bindings.get(SESSION_SOURCE, ""))
    return build_target(
        bindings.get(SESSION_PLATFORM, ""),
        bindings.get(SESSION_CHAT_ID, ""),
        bindings.get(SESSION_THREAD_ID, "") or "",
    )


def resolve_config() -> Dict[str, str]:
    """Read the plugin's configuration out of the environment, with defaults.

    Every value here is non-secret and ends up in an argv the owner runs. There
    is no credential among them on purpose: ``hermes send`` reads the platform
    tokens out of the profile that ``HERMES_HOME`` names, so this integration
    never holds one, never registers one, and never has one to leak.
    """
    home = os.environ.get("CFLX_HERMES_CALLBACK_HOME") or os.path.expanduser("~")
    hermes_home = (
        os.environ.get("CFLX_HERMES_HOME")
        or os.environ.get("HERMES_HOME")
        or os.path.join(home, ".hermes")
    )
    return {
        "cflx": os.environ.get("CFLX_BIN", "cflx"),
        # A *fallback* only, and only for a host that exposes no tool arguments
        # at all. Which owner a registration belongs to is decided by the
        # enqueue call that produced the execution, not by a process-global
        # variable that can name one project at a time; see `resolve_owner_route`.
        "unix_socket": os.environ.get("CFLX_UNIX_SOCKET", ""),
        "auth_token_env": os.environ.get("CFLX_AUTH_TOKEN_ENV", ""),
        "hermes": os.environ.get("CFLX_HERMES_BIN") or (shutil.which("hermes") or ""),
        "home": home,
        "path": os.environ.get("CFLX_HERMES_CALLBACK_PATH") or os.environ.get("PATH") or DEFAULT_SEARCH_PATH,
        "hermes_home": hermes_home,
        "python": sys.executable or "python3",
        "callback": os.environ.get("CFLX_HERMES_CALLBACK", CALLBACK_PATH),
    }


def build_callback_argv(config: Dict[str, str], target: str) -> list:
    """The argv the owner will execute, one element per argument.

    Never shell source. The interpreter, the callback and the Hermes executable
    are absolute, and the three environment values are spelled out, because the
    environment the owner hands the callback holds exactly five ``CFLX_*``
    variables and nothing else.
    """
    return [
        config["python"],
        config["callback"],
        "--hermes",
        config["hermes"],
        "--target",
        target,
        "--home",
        config["home"],
        "--path",
        config["path"],
        "--hermes-home",
        config["hermes_home"],
    ]


def build_registration_argv(config: Dict[str, str], binding: Dict[str, str], callback: list) -> list:
    """The ``cflx client notify set`` invocation that attaches the callback.

    Connection options belong to the ``client`` namespace, so they come before
    the verb. Exactly one route option is emitted, and it is the one the
    qualifying enqueue call itself selected: ``--project-dir`` for the normal
    project route, ``--unix-socket`` only when that low-level override was the
    call's own selector. Neither is ever omitted, because leaving both out would
    let the client derive a default from a working directory that is the Hermes
    gateway's rather than the project's. ``--auth-token-env`` names a variable;
    a token value is never accepted in argv and never printed.
    """
    argv = [
        config["cflx"],
        "client",
        ROUTE_OPTIONS[config["route_kind"]],
        config["route_value"],
    ]
    if config["auth_token_env"]:
        argv += ["--auth-token-env", config["auth_token_env"]]
    argv += [
        "notify",
        "set",
        binding["change_id"],
        binding["execution_id"],
        "--instance-id",
        binding["instance_id"],
        "--json",
        "--",
    ]
    return argv + callback


def _register(config: Dict[str, str], binding: Dict[str, str], target: str) -> None:
    """Attach one execution-scoped callback, or explain why nothing was attached."""
    argv = build_registration_argv(config, binding, build_callback_argv(config, target))
    try:
        completed = subprocess.run(  # noqa: S603 - a fixed argv, never shell source
            argv,
            capture_output=True,
            text=True,
            timeout=REGISTRATION_TIMEOUT_SECONDS,
        )
    except (OSError, subprocess.SubprocessError) as error:
        _warn("could not run '%s': %s" % (config["cflx"], error))
        return

    envelope: Optional[dict] = None
    for line in reversed(completed.stdout.splitlines()):
        line = line.strip()
        if not line:
            continue
        try:
            candidate = json.loads(line)
        except ValueError:
            continue
        if isinstance(candidate, dict):
            envelope = candidate
            break

    if envelope is None:
        _warn(
            "notify set produced no envelope (exit %d): %s"
            % (completed.returncode, (completed.stderr or "").strip()[:300])
        )
        return
    # Branch on `outcome`, never on prose or on the exit status alone.
    if envelope.get("ok") is not True or envelope.get("outcome") != SUBSCRIBED:
        _warn("notify set returned '%s'" % (envelope.get("outcome"),))
        return
    logger.debug(
        "cflx-auto-resume: registered a completion callback for %s/%s",
        binding["change_id"],
        binding["execution_id"],
    )


def _warn(message: str) -> None:
    """One diagnostic line. Never a credential, and never a raised exception.

    A hook that raised would be swallowed by Hermes anyway; saying so on stderr
    is what makes a missing notification diagnosable instead of silent.
    """
    logger.warning("cflx-auto-resume: %s", message)
    sys.stderr.write("cflx-auto-resume: %s\n" % (message,))


def on_post_tool_call(
    tool_name: str = "",
    args: Any = None,
    result: Any = None,
    **_: Any,
) -> None:
    """React to one completed tool call. Observational; it changes no result."""
    if not is_enqueue_tool(tool_name):
        return

    binding = extract_binding(result)
    if binding is None:
        # Not a supported, successful, admitted envelope. Registering nothing is
        # the whole point: there is no execution episode to bind a sink to.
        return

    config = resolve_config()
    try:
        target = resolve_target(read_session_bindings())
        # Which owner, from this call's own arguments. `args` is the only thing
        # in scope that knows: the result names an execution episode, and an
        # execution ID is process-local to the owner that admitted it, so the
        # same ID registered against a different owner is a different episode
        # or none at all. A call that named no route at all is refused here
        # rather than answered from `CFLX_UNIX_SOCKET`, which describes one
        # project and would silently misroute every other.
        config["route_kind"], config["route_value"] = resolve_owner_route(
            args, config["unix_socket"]
        )
        # The destination and the environment are proven once, here, rather than
        # at delivery time: the registered argv carries them out of this process,
        # and an operator watching a hook fail can fix a registration that a
        # one-shot callback failing hours later cannot tell anybody about.
        require_executable(config["hermes"])
        delivery_environment(config["home"], config["path"], config["hermes_home"])
    except ValueError as error:
        _warn("not registering a callback: %s" % (error,))
        return

    _register(config, binding, target)


def register(ctx) -> None:
    """Plugin entry point. One hook, no tools, no commands."""
    ctx.register_hook("post_tool_call", on_post_tool_call)


__all__ = [
    "CALLBACK_ENVIRONMENT_KEYS",
    "CALL_PROJECT_ARGUMENT",
    "CALL_SOCKET_ARGUMENT",
    "PROJECT_ROUTE",
    "ROUTE_OPTIONS",
    "SOCKET_ROUTE",
    "build_callback_argv",
    "build_registration_argv",
    "on_post_tool_call",
    "read_session_bindings",
    "register",
    "resolve_config",
    "resolve_owner_route",
    "resolve_target",
]
