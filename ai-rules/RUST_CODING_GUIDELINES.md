---
description: rust coding guidelines
alwaysApply: false
fileMatching: "**/*.rs"
---
# Safety-Critical Rust Coding Guidelines

> Source: [Safety-Critical Rust Coding Guidelines](https://coding-guidelines.arewesafetycriticalyet.org/)
> Version: 0.1
> Copyright: 2025, Contributors to Coding Guidelines Subcommittee

## Table of Contents

- [Introduction](#introduction)
- [Compliance](#compliance)
- [Types and Traits](#types-and-traits)
- [Expressions](#expressions)
- [Associated Items](#associated-items)
- [Macros](#macros)

---

## Introduction

Welcome to the Safety-Critical Rust Coding Guidelines. This document provides coding standards for Rust programming in safety-critical systems where reliability and safety are paramount concerns.

### Compliance

These guidelines follow **MISRA Compliance 2020**.

### Guideline Categories

- **Mandatory**: Must always be followed
- **Required**: Must be followed unless deviation is documented
- **Advisory**: Should be followed as best practice

### Guideline Status

- **Draft**: Under development
- **Released**: Finalized for use

---

## Types and Traits

### Guideline: Use Strong Types to Differentiate Logically Distinct Values

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Advisory |
| Scope | Module |

#### Rule

Parameters and variables with logically distinct types must be statically distinguishable by the type system.

#### When to Use Newtypes

Apply the newtype pattern (e.g., `struct Meters(u32)`) when:

- Multiple quantities share the same primitive representation but carry different semantic meaning
- Confusing them would create semantic errors
- Enhanced type safety and encapsulation is needed
- Trait-based differentiation is required
- New invariants must be established

#### Rationale

This approach prevents mixing semantically different values that share identical primitives. Benefits include:

- **Static Safety**: Compiler enforces domain distinctions at compile time
- **Readability**: Intent-revealing type names make code self-documenting
- **Domain Logic**: Enables trait implementations matching domain-specific behavior
- **API Evolution**: Strong contracts allow independent representation changes

#### Non-Compliant Example 1

Using primitive types directly allows parameter swapping:

```rust
fn travel(distance: u32, time: u32) -> u32 {
    distance / time
}

fn main() {
    let d = 100;
    let t = 10;
    let _result = travel(t, d);  // Compiles, but semantically incorrect
}
```

#### Non-Compliant Example 2

Type aliases don't create distinct types—compiler cannot enforce distinctions:

```rust
type Meters = u32;
type Seconds = u32;

fn travel(distance: Meters, time: Seconds) -> u32 {
    distance / time
}

fn main() {
    let d: Meters = 100;
    let t: Seconds = 10;
    let _result = travel(t, d);  // Still compiles incorrectly
}
```

#### Compliant Example

Newtypes provide compiler-enforced type safety with trait implementations:

```rust
use std::ops::Div;

#[derive(Debug, Clone, Copy)]
struct Meters(u32);
struct Seconds(u32);
struct MetersPerSecond(u32);

impl Div<Seconds> for Meters {
    type Output = MetersPerSecond;
    fn div(self, rhs: Seconds) -> Self::Output {
        MetersPerSecond(self.0 / rhs.0)
    }
}

fn main() {
    let d = Meters(100);
    let t = Seconds(10);
    let result = d / t;  // Type-safe and clean
    println!("Speed: {} m/s", result.0);
}
```

---

### Guideline: Ensure Union Field Reads Produce Valid Values

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Required |
| Scope | System |
| Decidability | Undecidable |

#### Rule

Ensure that the underlying bytes constitute a valid value for that field's type when reading from a union field.

Reading union fields whose bytes don't represent valid values for the target type causes undefined behavior.

#### Pre-Read Verification Requirements

Before accessing a union field, verify that:

- The union was last written through that specific field, OR
- It was written through a field whose bytes remain valid when reinterpreted as the target field type

Use explicit validity checks if the active field is uncertain.

#### Validity Invariants by Type

| Type | Valid Values |
|------|--------------|
| `bool` | Only `0` (false) or `1` (true) |
| `char` | Valid Unicode scalar values (`0x0`–`0xD7FF` or `0xE000`–`0x10FFFF`) |
| References | Must be non-null and properly aligned |
| Enums | Must hold valid discriminant values |
| Floating Point | All bit patterns valid for `f32`/`f64` |
| Integers | All bit patterns valid for integer types |

#### Non-Compliant Example: Invalid bool read

```rust
union IntOrBool {
    i: u8,
    b: bool,
}

fn main() {
    let u = IntOrBool { i: 3 };
    unsafe { u.b };  // UB: 3 is not valid bool value
}
```

#### Non-Compliant Example: Invalid Unicode read

```rust
union IntOrChar {
    i: u32,
    c: char,
}

fn main() {
    let u = IntOrChar { i: 0xD800 };  // Surrogate value
    unsafe { u.c };  // UB: Invalid Unicode scalar
}
```

#### Non-Compliant Example: Invalid enum discriminant read

```rust
#[repr(u8)]
enum Color { Red = 0, Green = 1, Blue = 2 }

union IntOrColor {
    i: u8,
    c: Color,
}

fn main() {
    let u = IntOrColor { i: 42 };
    unsafe { u.c };  // UB: 42 is invalid Color discriminant
}
```

#### Non-Compliant Example: Invalid reference read

```rust
union PtrOrRef {
    p: *const i32,
    r: &'static i32,
}

fn main() {
    let u = PtrOrRef { p: std::ptr::null() };
    unsafe { u.r };  // UB: Null pointer, not valid reference
}
```

#### Compliant Example 1: Active Field Tracking

Wrap union with explicit field tracking:

```rust
#[repr(C)]
#[derive(Copy, Clone)]
union IntOrBoolData {
    i: u8,
    b: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ActiveField { Int, Bool }

pub struct IntOrBool {
    data: IntOrBoolData,
    active: ActiveField,
}

impl IntOrBool {
    pub fn from_int(value: u8) -> Self {
        Self {
            data: IntOrBoolData { i: value },
            active: ActiveField::Int,
        }
    }

    pub fn as_int(&self) -> Option<u8> {
        match self.active {
            ActiveField::Int => Some(unsafe { self.data.i }),
            ActiveField::Bool => None,
        }
    }
}
```

#### Compliant Example 2: Reading Same Field Written

```rust
#[repr(C)]
#[derive(Copy, Clone)]
union IntBytes {
    i: u32,
    bytes: [u8; 4],
}

fn get_int() -> u32 {
    let u = IntBytes { i: 0x12345678 };
    // Safe: All bit patterns valid for [u8; 4]
    assert_eq!(unsafe { u.bytes }, 0x12345678_u32.to_ne_bytes());

    let u2 = IntBytes { bytes: [0x11, 0x22, 0x33, 0x44] };
    // Safe: All bit patterns valid for u32
    unsafe { u2.i }
}
```

#### Compliant Example 3: Validation Before Constrained Type Read

```rust
#[repr(C)]
union IntOrBool {
    i: u8,
    b: bool,
}

fn try_read_bool(u: &IntOrBool) -> Option<bool> {
    let raw = unsafe { u.i };  // Safe: all u8 patterns valid

    match raw {
        0 => Some(false),
        1 => Some(true),
        _ => None,  // Reject invalid patterns
    }
}
```

#### Compliant Example 4: Type-Safe FFI Wrapper

A comprehensive pattern using generics to track active field at compile time while maintaining FFI compatibility:

```rust
use std::marker::PhantomData;

pub struct AsInt;
pub struct AsBool;

#[repr(C)]
#[derive(Copy, Clone)]
pub union IntOrBoolData {
    pub i: u8,
    pub b: bool,
}

#[repr(C)]
pub struct IntOrBool<T> {
    data: IntOrBoolData,
    _marker: PhantomData<T>,
}

impl IntOrBool<AsInt> {
    pub fn from_int(value: u8) -> Self {
        Self {
            data: IntOrBoolData { i: value },
            _marker: PhantomData,
        }
    }

    pub fn get(&self) -> u8 {
        unsafe { self.data.i }  // Type parameter guarantees safety
    }
}

impl IntOrBool<AsBool> {
    pub fn from_bool(value: bool) -> Self {
        Self {
            data: IntOrBoolData { b: value },
            _marker: PhantomData,
        }
    }

    pub fn get(&self) -> bool {
        unsafe { self.data.b }  // Type parameter guarantees safety
    }
}
```

---

## Expressions

### Guideline: Do Not Use Integer Type as Divisor During Integer Division

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Advisory |

#### Rule

Never provide a right operand of integer type during division or remainder expressions when the left operand is also an integer type.

Applies to: `i8`, `i16`, `i32`, `i64`, `i128`, `u8`, `u16`, `u32`, `u64`, `u128`, `usize`, `isize`

#### Rationale

Division by zero causes panics. Use checked operations instead.

#### Compliant Approaches

- Use `checked_div()` and `checked_rem()` methods
- Wrap divisors with `std::num::NonZero` to guarantee non-zero values

```rust
// Using checked_div
let result = numerator.checked_div(denominator);

// Using NonZero
use std::num::NonZeroU32;
let divisor = NonZeroU32::new(value).expect("divisor must be non-zero");
let result = numerator / divisor;
```

---

### Guideline: The 'as' Operator Should Not Be Used with Numeric Operands

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Advisory |

#### Rule

Avoid `as` casts between numeric types, bool, or char. Use `Into`, `From`, `TryInto`, or `TryFrom` instead.

**Exception**: `as usize` with raw pointer types is permitted.

#### Rationale

The `as` operator may cause data loss through truncation or rounding without communicating intent. Traits better signal whether conversions are lossless or fallible.

#### Compliant Alternatives

```rust
// Lossless conversion
let value: u32 = small_value.into();

// Fallible conversion
let value: u8 = large_value.try_into()?;

// Explicit type conversion
let float = f64::from(integer);
```

---

### Guideline: Avoid Underscore Pointer Casts

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Required |

#### Rule

Specify complete target types explicitly in pointer casts. Avoid relying on type inference with `_` placeholders.

#### Rationale

Inferring pointer types can lead to unintended type changes if surrounding code modifications occur, resulting in semantically invalid casts.

#### Non-Compliant Example

```rust
let ptr = extended as *const _;  // Type inferred - dangerous
```

#### Compliant Example

```rust
let ptr = extended as *const Extended;  // Type explicit - safe
```

---

### Guideline: Avoid Out-of-Range Shifts

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Mandatory |

#### Rule

Do not shift by negative values or by amounts ≥ the operand's bit width.

Applies to all primitive integer types.

#### Compliant Methods

```rust
// Use checked_shl (returns Option)
let result = value.checked_shl(shift_amount);

// Validate shift amounts
if shift >= 0 && shift < 32 {
    let result = value << shift;
}

// Use overflowing_shl to detect out-of-range conditions
let (result, overflowed) = value.overflowing_shl(shift_amount);
```

---

### Guideline: An Integer Shall Not Be Converted to a Pointer

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Required |

#### Rule

Never use `as` operator or `transmute` to convert numeric types to pointers.

#### Rationale

Converting arbitrary integers to pointers creates invalid memory addresses, risking undefined behavior.

#### Compliant Alternative

Use `core::ptr::null()` or `core::ptr::null_mut()` for null pointers:

```rust
let null_ptr: *const i32 = core::ptr::null();
let null_mut_ptr: *mut i32 = core::ptr::null_mut();
```

---

### Guideline: Do Not Shift by Negative Numbers or ≥ Bitwidth

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Advisory |

#### Rule

Verify shift amounts before operations. Use `checked_shl()` exclusively to eliminate out-of-range shifts entirely.

```rust
// Always use checked shift operations
if let Some(result) = value.checked_shl(shift_amount) {
    // Use result
}
```

---

### Guideline: Ensure Integer Operations Do Not Result in Arithmetic Overflow

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Required |
| Scope | System |

#### Rule

Eliminate overflow for both signed and unsigned integers. Use explicit wrapping only when intentional.

#### Compliant Strategies

```rust
// Range checking before operations
if (si_b > 0 && si_a > i32::MAX - si_b) || (si_b < 0 && si_a < i32::MIN - si_b) {
    return Err(ArithmeticError::Overflow);
}

// Using overflowing methods
let (result, overflowed) = a.overflowing_add(b);
if overflowed {
    return Err(ArithmeticError::Overflow);
}

// Intentional wraparound with Wrapping<T>
use std::num::Wrapping;
let a = Wrapping(200u8);
let b = Wrapping(100u8);
let result = a + b;  // Wraps intentionally

// Saturating arithmetic
use std::num::Saturating;
let a = Saturating(200u8);
let b = Saturating(100u8);
let result = a + b;  // Saturates at max value
```

#### Summary Table

| Operation | Safe Method |
|-----------|-------------|
| Division | `checked_div()` |
| Type casting | `Into`/`TryInto` traits |
| Pointer casts | Explicit target types |
| Shift operations | `checked_shl()`/`checked_shr()` |
| Integer-to-pointer | Prohibited |
| Arithmetic | `overflowing_*()` or `Wrapping<T>` |

---

## Associated Items

### Guideline: Recursive Functions Are Not Allowed

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Required |
| Tags | stack-overflow |
| Scope | System |
| Decidability | Undecidable |

#### Rule

Any function shall not call itself directly or indirectly.

#### Rationale

Recursive functions pose significant risks in safety-critical systems. Such functions can easily trigger stack overflows, potentially causing exceptions or undefined behavior—particularly problematic in embedded systems environments.

While Rust's compiler supports tail call optimization, this feature remains unguaranteed and depends on implementation specifics. Until tail call optimization is guaranteed and stabilized, developers should avoid recursion to prevent stack overflows and maintain program reliability.

#### Non-Compliant Example

```rust
enum MyEnum {
    String(String),
    List(Vec<MyEnum>),
}

fn concat_strings(value: &MyEnum) -> String {
    match value {
        MyEnum::String(s) => s.clone(),
        MyEnum::List(items) => {
            let mut result = String::new();
            for item in items {
                result.push_str(&concat_strings(item));  // Recursive call - dangerous
            }
            result
        }
    }
}
```

#### Compliant Example

Replace recursion with iteration using an explicit stack:

```rust
enum MyEnum {
    String(String),
    List(Vec<MyEnum>),
}

const MAX_STACK_SIZE: usize = 1000;

fn concat_strings(value: &MyEnum) -> Result<String, &'static str> {
    let mut result = String::new();
    let mut stack: Vec<&MyEnum> = vec![value];

    while let Some(current) = stack.pop() {
        if stack.len() >= MAX_STACK_SIZE {
            return Err("Stack overflow: nesting too deep");
        }

        match current {
            MyEnum::String(s) => result.push_str(s),
            MyEnum::List(items) => {
                // Push items in reverse order to process left-to-right
                for item in items.iter().rev() {
                    stack.push(item);
                }
            }
        }
    }

    Ok(result)
}
```

Key features of the compliant approach:

- Manual stack management for processing context
- Predefined `MAX_STACK_SIZE` constant
- Error return if stack exceeds limits rather than risking overflow
- Predictable, robust behavior in resource-constrained environments

---

## Macros

### Guideline: Attribute Macros Shall Not Be Used

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Required |

#### Rule

Attribute macros shall neither be declared nor invoked.

#### Rationale

Attribute macros can rewrite items unexpectedly, causing confusion and introducing errors.

#### Non-Compliant Example

```rust
#[test]  // Attribute macro
fn test_something() {
    assert!(true);
}
```

---

### Guideline: A Macro Should Not Be Used in Place of a Function

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Mandatory |

#### Rule

Functions should be preferred except when macros provide essential functionality:

- Variadic interfaces
- Compile-time code generation
- Custom derives

#### Rationale

Macros complicate debugging, may inhibit optimization, and lack clear type signatures.

#### Non-Compliant Example

```rust
macro_rules! increment {
    ($x:expr) => {
        $x += 1  // Implicit mutation without clear signature
    };
}

let mut value = 5;
increment!(value);
```

#### Compliant Example

```rust
fn increment(x: &mut i32) {
    *x += 1  // Explicit borrowing and mutation
}

let mut value = 5;
increment(&mut value);
```

---

### Guideline: Procedural Macros Should Not Be Used

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Advisory |

#### Rule

Macros should be expressed using declarative syntax in preference to procedural syntax.

#### Rationale

Procedural macros contain arbitrary Rust code, making them harder to understand and potentially unsafe.

---

### Guideline: Ensure Complete Hygiene of Macros

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Mandatory |

#### Rule

Ensure macros do not capture or interfere with identifiers from the calling scope unintentionally.

---

### Guideline: Do Not Hide Unsafe Blocks Within Macro Expansions

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Required |

#### Rule

Unsafe blocks must not be hidden within macro expansions.

#### Rationale

Hidden unsafe code bypasses code review scrutiny and makes reasoning about safety properties difficult.

---

### Guideline: Names in a Macro Definition Shall Use Fully Qualified Paths

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Required |

#### Rule

Each name inside of the definition of a macro shall either use a global path or path prefixed with `$crate`.

#### Rationale

This prevents shadowing and ensures consistent behavior across different usage contexts.

#### Non-Compliant Example

```rust
macro_rules! create_vec {
    () => {
        Vec::new()  // May be shadowed
    };
}
```

#### Compliant Example

```rust
macro_rules! create_vec {
    () => {
        ::std::vec::Vec::new()  // Unambiguous resolution
    };
}
```

---

### Guideline: Shall Not Use Function-like Macros

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Mandatory |

#### Rule

Function-like macros should be avoided in favor of regular functions when possible.

---

### Guideline: Shall Not Invoke Macros

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Mandatory |

#### Rule

Macro invocation should be minimized in safety-critical code.

---

### Guideline: Shall Not Use Declarative Macros

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Mandatory |

#### Rule

Declarative macros (`macro_rules!`) should be avoided when functions can achieve the same result.

---

### Guideline: Shall Not Write Code That Expands Macros

| Property | Value |
|----------|-------|
| Status | Draft |
| Category | Mandatory |

#### Rule

Minimize reliance on macro expansion in safety-critical codebases.

---

## Additional Sections (Placeholders)

The following sections exist in the guidelines but currently contain no content:

- **Patterns**
- **Values**
- **Statements**
- **Functions**
- **Implementations**
- **Generics**
- **Attributes**
- **Entities and Resolution**
- **Ownership and Destruction**
- **Exceptions and Errors**
- **Concurrency**
- **Program Structure and Compilation**
- **Unsafety**
- **FFI**
- **Inline Assembly**

These sections may be populated in future versions of the guidelines.

---

## References

- [MISRA Compliance 2020](https://www.misra.org.uk/)
- [Rust Reference - Unsafe Code](https://doc.rust-lang.org/reference/unsafety.html)
- [Rust Nomicon](https://doc.rust-lang.org/nomicon/)
