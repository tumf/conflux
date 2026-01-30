# Refactoring Guidelines for LLM-Assisted Development

Safe, disciplined refactoring using small, verifiable steps. Based on insights from "Refactoring" (Fowler) and "Working Effectively with Legacy Code" (Feathers).

## Core Philosophy

**The goal of refactoring is to change code structure without changing behavior.** Every step should be small enough to verify correctness before proceeding.

### Edit and Pray vs Cover and Modify

When you have good test coverage: make changes confidently, run tests after each step.

When you don't have tests (the common case): use only transformations that can be verified through:
- Visual inspection (the before/after are obviously equivalent)
- Compiler/type checker (rename errors catch missing references)
- Mechanical transformation (no judgment calls, just moving code)

**If you can't verify a step, make it smaller.**

## The Golden Rule: One Thing at a Time

Never mix:
- Refactoring with behavior changes
- Multiple refactoring operations
- Cleanup with feature work

Each commit/step should be a single, reversible transformation.

## Safe Refactoring Catalog

These transformations are safe because they can be verified without tests.

### Extract Method/Function

**When to use:** Code is too long, has comments explaining sections, or you need to reuse a portion.

**Why it's safe:** The extracted code is literally copy-pasted. Compiler catches missing variables.

```python
# Before
def process_order(order):
    # validate
    if order.total < 0:
        raise ValueError("Invalid total")
    if not order.items:
        raise ValueError("No items")

    # calculate discount
    discount = 0
    if order.total > 100:
        discount = order.total * 0.1

    return order.total - discount

# After
def process_order(order):
    validate_order(order)
    discount = calculate_discount(order.total)
    return order.total - discount

def validate_order(order):
    if order.total < 0:
        raise ValueError("Invalid total")
    if not order.items:
        raise ValueError("No items")

def calculate_discount(total):
    if total > 100:
        return total * 0.1
    return 0
```

```typescript
// Before
function processOrder(order: Order): number {
  // validate
  if (order.total < 0) throw new Error("Invalid total");
  if (!order.items.length) throw new Error("No items");

  // calculate discount
  let discount = 0;
  if (order.total > 100) {
    discount = order.total * 0.1;
  }

  return order.total - discount;
}

// After
function processOrder(order: Order): number {
  validateOrder(order);
  const discount = calculateDiscount(order.total);
  return order.total - discount;
}

function validateOrder(order: Order): void {
  if (order.total < 0) throw new Error("Invalid total");
  if (!order.items.length) throw new Error("No items");
}

function calculateDiscount(total: number): number {
  return total > 100 ? total * 0.1 : 0;
}
```

### Inline Method/Function

**When to use:** A function's body is as clear as its name, or you need to see the full picture before re-extracting differently.

**Why it's safe:** Mechanical replacement, compiler catches type mismatches.

```go
// Before
func isAdult(age int) bool {
    return age >= 18
}

func canVote(person Person) bool {
    return isAdult(person.Age) && person.Registered
}

// After (if isAdult adds no clarity)
func canVote(person Person) bool {
    return person.Age >= 18 && person.Registered
}
```

### Rename (Variable, Function, Class, File)

**When to use:** Name doesn't reflect purpose, or purpose has changed.

**Why it's safe:** Compiler/linter catches all references. Find-and-replace with whole-word matching.

**Caution:** Watch for:
- String references (API endpoints, serialization)
- Dynamic access (`obj[fieldName]`)
- Cross-file public APIs

### Move Method/Function

**When to use:** Function is more closely related to another module/class.

**Why it's safe:** Import errors catch missing references.

```dart
// Before: in utils.dart
String formatCurrency(double amount) {
  return '\$${amount.toStringAsFixed(2)}';
}

// After: moved to money.dart (where Money class lives)
// Update all imports from 'utils.dart' to 'money.dart'
```

### Extract Variable

**When to use:** Expression is complex or used multiple times.

**Why it's safe:** Pure mechanical extraction, no logic change.

```python
# Before
if user.subscription.plan.price > 100 and user.subscription.plan.price < 500:
    apply_mid_tier_discount(user.subscription.plan.price)

# After
price = user.subscription.plan.price
if price > 100 and price < 500:
    apply_mid_tier_discount(price)
```

### Inline Variable

**When to use:** Variable adds no explanatory value.

**Why it's safe:** Direct substitution.

```typescript
// Before
const basePrice = order.basePrice;
return basePrice;

// After
return order.basePrice;
```

### Split Loop

**When to use:** Loop does multiple unrelated things.

**Why it's safe:** Same iterations, same operations, just separated.

```go
// Before
var sum int
var product int = 1
for _, v := range values {
    sum += v
    product *= v
}

// After
var sum int
for _, v := range values {
    sum += v
}

var product int = 1
for _, v := range values {
    product *= v
}
```

**Note:** Yes, this is "slower" (two loops). Optimize later if profiling shows it matters. Clarity enables further refactoring.

### Replace Nested Conditional with Guard Clauses

**When to use:** Deep nesting obscures the main logic.

**Why it's safe:** Same conditions, same outcomes, different structure.

```python
# Before
def get_payment_amount(employee):
    if employee.is_separated:
        result = separated_amount(employee)
    else:
        if employee.is_retired:
            result = retired_amount(employee)
        else:
            result = normal_amount(employee)
    return result

# After
def get_payment_amount(employee):
    if employee.is_separated:
        return separated_amount(employee)
    if employee.is_retired:
        return retired_amount(employee)
    return normal_amount(employee)
```

## Temporarily Making Code Worse

Sometimes you need to make code worse before making it better. This is expected and safe.

### Duplicate Before Unifying

To extract common code from two places:
1. First, make them identical (even if that means temporary duplication/ugliness)
2. Then extract the common code

```typescript
// Two similar but different functions
function processUserOrder(order: Order) {
  validateUser(order.userId);
  const tax = order.total * 0.08;
  const shipping = 5.99;
  return order.total + tax + shipping;
}

function processGuestOrder(order: Order) {
  const tax = order.total * 0.08;
  const shipping = order.items > 2 ? 0 : 5.99;  // different!
  return order.total + tax + shipping;
}

// Step 1: Make shipping calculation explicit in both (worse but parallel)
function processUserOrder(order: Order) {
  validateUser(order.userId);
  const tax = order.total * 0.08;
  const shipping = calculateShipping(order, false);
  return order.total + tax + shipping;
}

function processGuestOrder(order: Order) {
  const tax = order.total * 0.08;
  const shipping = calculateShipping(order, true);
  return order.total + tax + shipping;
}

// Step 2: Now extract the common part
function calculateOrderTotal(order: Order, isGuest: boolean): number {
  const tax = order.total * 0.08;
  const shipping = calculateShipping(order, isGuest);
  return order.total + tax + shipping;
}
```

### Expand Before Contracting

To simplify complex conditionals:
1. First, expand to explicit cases (more verbose)
2. Then identify patterns and simplify

## Seams: Places to Change Behavior Without Editing

A "seam" is a place where you can alter program behavior without modifying the code at that location.

### Object Seam (Most Common)

Pass a dependency rather than hard-coding it.

```python
# Before: hard to test or modify behavior
def send_notification(user, message):
    client = SMTPClient("mail.server.com")
    client.send(user.email, message)

# After: seam at the client parameter
def send_notification(user, message, client=None):
    if client is None:
        client = SMTPClient("mail.server.com")
    client.send(user.email, message)
```

**Note:** This isn't about creating interfaces for everything. It's about having one place to inject different behavior when needed (testing, debugging, gradual migration).

### Configuration Seam

Behavior controlled by configuration rather than code.

```go
// Seam: behavior changes based on config, not code edits
func ProcessPayment(amount float64) error {
    if config.PaymentProvider == "stripe" {
        return processStripe(amount)
    }
    return processSquare(amount)
}
```

## Refactoring Workflow for LLMs

**Critical: Do not plan and execute all refactorings at once.** That's equivalent to a rewrite, which defeats the purpose of safe refactoring.

The loop is:
1. **Refactor** - One transformation
2. **Test/Evaluate** - Run tests, check compiler, verify behavior
3. **Decide** - Based on how the code looks now, what's the next step?

Observe how the code evolves. The right next step often becomes clear only after the previous step is complete.

### Verification Methods

- **Compiler/type checker** - Catches missing references, type mismatches
- **Unit tests** - Fast feedback on behavior preservation
- **Visual tests** - Screenshots (web), golden tests (Flutter) provide extra safety net for UI changes—but only if they're fast to run. Judgment call.
- **Manual inspection** - For simple transformations, before/after should be obviously equivalent

### Calibrating Step Size

How big a step can you safely take? It depends on:

- **Test coverage** - More tests = larger safe steps
- **Type system strength** - Static typing catches more errors
- **Your capability level** - Check benchmarks like SWE-bench verified to understand your own performance. More capable = larger steps. If uncertain about your capability, ask the user what model you are.

When in doubt, smaller steps. You can always go faster once you've verified the approach works.

### Why Smaller Functions Help LLMs

LLMs can handle more code at once than humans, but there are practical limits:

- **Search/replace tools struggle with deep indentation** - Extracting methods reduces nesting, making edits more reliable
- **Smaller functions = smaller context needed** - Easier to reason about in isolation
- **Flat structure = fewer edit conflicts** - Deeply nested code requires more precise targeting

The human "one page rule" extends to roughly what fits comfortably in your context window, but prefer flatter structure regardless.

### Example: Breaking Up a Large Function

```
Step 1: Extract lines 15-30 → validateInput()
  → Run tests, commit
  → Observe: the remaining function is clearer now

Step 2: Extract lines 45-60 → calculateTotals()
  → Run tests, commit
  → Observe: there's a pattern emerging

Step 3: Extract lines 70-90 → formatOutput()
  → Run tests, commit
  → Decide: is the core logic clear now, or does it need more work?
```

Each step is independently verifiable. If step 3 breaks something, you know exactly where. The next step is decided based on what the code looks like now, not planned upfront.

## What NOT to Do

### Don't Mix Refactoring with Features
```
❌ "While extracting this method, I also fixed the edge case..."
✓ "Extract method (commit). Now fix edge case (separate commit)."
```

### Don't Create Speculative Abstractions
```
❌ Extract interface so "someday we might swap implementations"
✓ Extract interface only when you have two implementations NOW
```

### Don't Refactor Without Purpose
```
❌ "This could be cleaner..." (then leave 5 half-finished changes)
✓ Finish what you start, or don't start
```

### Don't Skip the Verification Step
```
❌ Make 10 changes, then run tests
✓ Make 1 change, verify, repeat
```

## Quick Reference: When to Use What

| Situation | Transformation |
|-----------|---------------|
| Long function | Extract Method |
| Unclear name | Rename |
| Complex expression | Extract Variable |
| Code in wrong file | Move Function |
| Deep nesting | Guard Clauses |
| Loop does too much | Split Loop |
| Two similar functions | Duplicate → Unify |
| Need test seam | Add optional parameter |
