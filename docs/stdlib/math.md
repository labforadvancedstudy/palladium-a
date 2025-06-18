# Math Module Reference

This page documents the mathematical functions available in the Palladium standard library.

## Import
```palladium
import std::math;
```

## Functions

### pd_abs
```palladium
fn pd_abs(x: i64) -> i64
```
Returns the absolute value of an integer.

**Parameters:**
- `x`: The integer value

**Returns:**
- The absolute value of x

**Example:**
```palladium
let positive = pd_abs(-42);    // 42
let unchanged = pd_abs(42);    // 42
let zero = pd_abs(0);          // 0
```

### max
```palladium
fn max(a: i64, b: i64) -> i64
```
Returns the larger of two integers.

**Parameters:**
- `a`: First integer
- `b`: Second integer

**Returns:**
- The larger value between a and b

**Example:**
```palladium
let larger = max(10, 20);      // 20
let same = max(5, 5);          // 5
let negative = max(-10, -5);   // -5
```

### min
```palladium
fn min(a: i64, b: i64) -> i64
```
Returns the smaller of two integers.

**Parameters:**
- `a`: First integer
- `b`: Second integer

**Returns:**
- The smaller value between a and b

**Example:**
```palladium
let smaller = min(10, 20);     // 10
let same = min(5, 5);          // 5
let negative = min(-10, -5);   // -10
```

### pd_pow
```palladium
fn pd_pow(base: i64, exp: i64) -> i64
```
Raises a base to an integer power.

**Parameters:**
- `base`: The base value
- `exp`: The exponent (must be non-negative)

**Returns:**
- base raised to the power of exp

**Note:** Returns 1 for exp = 0, returns 0 for negative exponents (should be 1/base^|exp| but we don't have floats)

**Example:**
```palladium
let square = pd_pow(2, 8);     // 256
let cube = pd_pow(3, 3);       // 27
let one = pd_pow(42, 0);       // 1 (any number to power 0)
let base_one = pd_pow(1, 100); // 1 (1 to any power)
```

## Complete Examples

### Distance Calculation
```palladium
import std::math;

fn manhattan_distance(x1: i64, y1: i64, x2: i64, y2: i64) -> i64 {
    let dx = pd_abs(x2 - x1);
    let dy = pd_abs(y2 - y1);
    return dx + dy;
}

fn chebyshev_distance(x1: i64, y1: i64, x2: i64, y2: i64) -> i64 {
    let dx = pd_abs(x2 - x1);
    let dy = pd_abs(y2 - y1);
    return max(dx, dy);
}
```

### Clamping Values
```palladium
import std::math;

fn clamp(value: i64, min_val: i64, max_val: i64) -> i64 {
    return max(min_val, min(value, max_val));
}

fn normalize_percentage(value: i64) -> i64 {
    return clamp(value, 0, 100);
}
```

### Simple Statistics
```palladium
import std::math;
import stdlib::vec_simple;

fn calculate_range(data: VecInt) -> i64 {
    if vec_int_is_empty(data) {
        return 0;
    }
    
    let min_val = vec_int_min(data);
    let max_val = vec_int_max(data);
    return max_val - min_val;
}

fn calculate_midpoint(a: i64, b: i64) -> i64 {
    // Avoid overflow for large values
    return min(a, b) + pd_abs(a - b) / 2;
}
```

### Bit Manipulation
```palladium
import std::math;

fn count_bits(n: i64) -> i64 {
    let mut count = 0;
    let mut value = pd_abs(n);
    
    while value > 0 {
        if value % 2 == 1 {
            count = count + 1;
        }
        value = value / 2;
    }
    
    return count;
}

fn is_power_of_two(n: i64) -> bool {
    if n <= 0 {
        return false;
    }
    
    // A power of 2 in binary has exactly one bit set
    // We can check this by seeing if n & (n-1) == 0
    // But without bitwise ops, we use a different approach
    let mut value = n;
    while value > 1 {
        if value % 2 != 0 {
            return false;
        }
        value = value / 2;
    }
    return true;
}
```

### Geometric Calculations
```palladium
import std::math;

fn rectangle_area(width: i64, height: i64) -> i64 {
    return pd_abs(width) * pd_abs(height);
}

fn rectangle_perimeter(width: i64, height: i64) -> i64 {
    return 2 * (pd_abs(width) + pd_abs(height));
}

fn is_square(width: i64, height: i64) -> bool {
    return pd_abs(width) == pd_abs(height);
}
```

### Number Theory
```palladium
import std::math;

fn factorial(n: i64) -> i64 {
    if n < 0 {
        return 0;  // Undefined for negative numbers
    }
    if n == 0 || n == 1 {
        return 1;
    }
    
    let mut result = 1;
    let mut i = 2;
    while i <= n {
        result = result * i;
        i = i + 1;
    }
    return result;
}

fn gcd(a: i64, b: i64) -> i64 {
    let mut x = pd_abs(a);
    let mut y = pd_abs(b);
    
    while y != 0 {
        let temp = y;
        y = x % y;
        x = temp;
    }
    
    return x;
}

fn lcm(a: i64, b: i64) -> i64 {
    if a == 0 || b == 0 {
        return 0;
    }
    return pd_abs(a * b) / gcd(a, b);
}
```

### Digit Operations
```palladium
import std::math;

fn digit_sum(n: i64) -> i64 {
    let mut sum = 0;
    let mut value = pd_abs(n);
    
    while value > 0 {
        sum = sum + (value % 10);
        value = value / 10;
    }
    
    return sum;
}

fn digit_count(n: i64) -> i64 {
    if n == 0 {
        return 1;
    }
    
    let mut count = 0;
    let mut value = pd_abs(n);
    
    while value > 0 {
        count = count + 1;
        value = value / 10;
    }
    
    return count;
}

fn reverse_digits(n: i64) -> i64 {
    let mut result = 0;
    let mut value = pd_abs(n);
    
    while value > 0 {
        result = result * 10 + (value % 10);
        value = value / 10;
    }
    
    return result;
}
```

## Mathematical Constants

Since Palladium currently only supports integers, mathematical constants must be approximated:

```palladium
// Approximations (scaled by 1000 for 3 decimal places)
let PI_TIMES_1000 = 3142;      // π ≈ 3.142
let E_TIMES_1000 = 2718;       // e ≈ 2.718
let SQRT2_TIMES_1000 = 1414;   // √2 ≈ 1.414

// Usage example: calculate circle area
fn circle_area_times_1000(radius: i64) -> i64 {
    // Area = π * r²
    // Returns area * 1000 to maintain precision
    return PI_TIMES_1000 * radius * radius / 1000;
}
```

## Limitations

Current limitations due to integer-only arithmetic:
- No floating-point operations
- No trigonometric functions (sin, cos, tan)
- No logarithms or exponentials (except integer powers)
- No square roots (except perfect squares)
- Limited precision for division

## Workarounds

### Fixed-Point Arithmetic
```palladium
// Use scaling for decimal precision
fn divide_with_precision(dividend: i64, divisor: i64, scale: i64) -> i64 {
    if divisor == 0 {
        return 0;  // Error case
    }
    return (dividend * scale) / divisor;
}

// Example: 10 / 3 with 2 decimal places
let result = divide_with_precision(10, 3, 100);  // 333 (represents 3.33)
```

### Integer Square Root
```palladium
fn isqrt(n: i64) -> i64 {
    if n < 0 {
        return 0;  // Error case
    }
    if n < 2 {
        return n;
    }
    
    // Newton's method for integer square root
    let mut x = n;
    let mut y = (x + 1) / 2;
    
    while y < x {
        x = y;
        y = (x + n / x) / 2;
    }
    
    return x;
}
```

## Future Enhancements

Planned additions when the language supports floating-point:
- Trigonometric functions (sin, cos, tan, etc.)
- Logarithmic functions (log, ln, log10)
- Exponential function (exp)
- Precise square root
- Rounding functions (floor, ceil, round)
- Random number generation
- Statistical functions (mean, median, std_dev)