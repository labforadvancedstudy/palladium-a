# Dead Code Elimination Test Failures Analysis

## Summary

Two tests in `src/optimizer/dead_code_test.rs` are failing due to conflicting expectations about how aggressive dead code elimination should be.

## Failing Tests

### 1. test_complex_control_flow (line 315-360)

**Test Structure:**
```rust
while true {
    if x {
        break;      // Terminates
        1;          // Dead - correctly removed
    } else {
        continue;   // Terminates  
        2;          // Dead - correctly removed
    }
    3;              // Test expects this to remain, but it's unreachable
}
```

**Issue:** 
- Line 334 has comment: `// Reachable (if condition is false and no else)`
- But the code HAS an else branch that also terminates with `continue`
- Since both branches terminate, the expression `3` is actually unreachable
- The optimizer correctly identifies this as dead code but the test expects it to remain

**Test Expectation:** `body.len() == 2` (If statement + expression 3)
**Actual Result:** `body.len() == 1` (Only If statement, expression 3 removed)

### 2. test_nested_control_flow (line 420-479)

**Test Structure:**
```rust
while true {
    if cond1 {
        if cond2 {
            return;     // Terminates
            1;          // Dead - correctly removed
        } else {
            break;      // Terminates
            2;          // Dead - correctly removed
        }
        3;              // Test expects this to remain, but it's unreachable
    }
}
4;                      // Reachable after while loop breaks
```

**Issue:**
- Line 442 has comment: `// Reachable if inner if takes neither branch`
- But the inner if has BOTH then and else branches that terminate
- There's no path where "neither branch" is taken
- The optimizer correctly identifies expression `3` as dead code

**Test Expectation:** `then_branch.len() == 2` (Inner if + expression 3)
**Actual Result:** `then_branch.len() == 1` (Only inner if, expression 3 removed)

## Root Cause

Both tests have incorrect expectations about control flow analysis:

1. They assume code after an if statement is reachable even when both branches terminate
2. The comments suggest the test authors misunderstood the control flow
3. The dead code eliminator is working correctly according to standard control flow analysis

## Possible Solutions

1. **Fix the tests** (Recommended):
   - Update test expectations to match correct behavior
   - Fix misleading comments
   - Remove assertions expecting dead code to remain

2. **Make optimizer less aggressive**:
   - Only remove dead code after explicit terminators (return, break, continue)
   - Don't analyze if statements for termination
   - This would make the optimizer less effective

3. **Add optimizer configuration**:
   - Add levels of aggressiveness for dead code elimination
   - Allow tests to specify which level to use
   - More complex but provides flexibility

## Recommendation

The optimizer is working correctly. The tests have incorrect expectations and should be fixed to match the correct behavior. The dead code elimination pass is properly identifying unreachable code based on control flow analysis.