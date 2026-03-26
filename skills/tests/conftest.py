"""Configure sys.path so shared modules are importable from tests."""
import sys
from pathlib import Path

# skills/ directory
SKILLS_DIR = Path(__file__).resolve().parent.parent
if str(SKILLS_DIR) not in sys.path:
    sys.path.insert(0, str(SKILLS_DIR))
