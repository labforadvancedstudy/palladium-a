# Alan von Palladium v0.3 - The Real Deal TODO

```
 ╔══════════════════════════════════════════════════════════════╗
 ║              MISSION: TURING-COMPLETE LANGUAGE               ║
 ║         Loops, Arrays, Functions, and Real Programs          ║
 ╚══════════════════════════════════════════════════════════════╝
```

## 🎯 Goal: Turing-Complete Programming (2 weeks)

**Success Criteria**: Implement a Splay Tree in Palladium
```palladium
fn splay_tree_demo() {
    let mut tree = new_tree();
    
    // Insert values
    let values = [5, 3, 7, 1, 9, 4, 6];
    for val in values {
        tree = insert(tree, val);
    }
    
    // Search and splay
    let found = search(tree, 4);
    if found {
        print("Found 4, now at root!");
    }
    
    // In-order traversal
    print("Tree contents:");
    traverse(tree);
}
```

## 📋 TODO List (Priority Order)

### Week 1: Core Language Features

#### Day 1-2: While Loops
- [ ] Add `while` to AST
  ```rust
  Stmt::While {
      condition: Expr,
      body: Vec<Stmt>,
      span: Span,
  }
  ```
- [ ] Parse while syntax
- [ ] Type check loop condition (must be bool)
- [ ] Generate proper loop code with labels
- [ ] Test infinite loops and termination
- [ ] Add loop scope handling

#### Day 3-4: Variable Reassignment
- [ ] Add assignment statement to AST
  ```rust
  Stmt::Assign {
      target: String,
      value: Expr,
      span: Span,
  }
  ```
- [ ] Parse `variable = value;` syntax
- [ ] Check variable exists in scope
- [ ] Type check assignment compatibility
- [ ] Generate assignment code
- [ ] Add `mut` keyword for mutable variables

#### Day 5-6: Arrays
- [ ] Add array type to type system
  ```rust
  Type::Array(Box<Type>, usize), // element type, size
  ```
- [ ] Array literal syntax: `[1, 2, 3, 4, 5]`
- [ ] Array indexing: `arr[i]`
- [ ] Array assignment: `arr[i] = value`
- [ ] Bounds checking
- [ ] Array length: `arr.len()`

### Week 2: Functions and Advanced Features

#### Day 7-8: Function Parameters
- [ ] Update function AST to support parameters
  ```rust
  params: Vec<(String, Type)>, // name, type
  ```
- [ ] Parse function parameters
- [ ] Type check function calls with arguments
- [ ] Generate code for parameter passing
- [ ] Support return values from any function
- [ ] Recursive function calls

#### Day 9-10: For Loops
- [ ] Add for loop to AST
  ```rust
  Stmt::For {
      init: Box<Stmt>,
      condition: Expr,
      update: Box<Stmt>,
      body: Vec<Stmt>,
      span: Span,
  }
  ```
- [ ] Parse C-style for loops
- [ ] For-in loops for arrays
- [ ] Generate optimized loop code

#### Day 11-12: Break/Continue
- [ ] Add break/continue to AST
- [ ] Track loop nesting level
- [ ] Generate proper jump instructions
- [ ] Ensure type safety (no value from break)
- [ ] Test nested loop scenarios

#### Day 13-14: Integration & Examples
- [ ] Implement bubble sort
- [ ] Implement binary search
- [ ] Implement linked list operations
- [ ] Implement tree traversal
- [ ] **Implement Splay Tree!**
- [ ] Performance benchmarks

## 🚀 New Language Features for v0.3

### Supported:
- [x] While loops with proper scoping
- [x] Mutable variables and reassignment
- [x] Fixed-size arrays
- [x] Array indexing and modification
- [x] Functions with parameters
- [x] Return from any function
- [x] For loops (C-style and for-in)
- [x] Break and continue statements
- [x] Recursive functions

### Still Deferred (v0.4+):
- [ ] Dynamic arrays/vectors
- [ ] Strings as first-class types
- [ ] Structs and enums
- [ ] Pattern matching
- [ ] Closures
- [ ] Generics
- [ ] Modules and imports
- [ ] Memory management

## 📁 Example Programs to Implement

### Sorting Algorithm
```palladium
fn bubble_sort(mut arr: [i32; 10]) -> [i32; 10] {
    let n = arr.len();
    for i in 0..n {
        for j in 0..(n-i-1) {
            if arr[j] > arr[j+1] {
                let temp = arr[j];
                arr[j] = arr[j+1];
                arr[j+1] = temp;
            }
        }
    }
    return arr;
}
```

### Binary Search
```palladium
fn binary_search(arr: [i32; 10], target: i32) -> i32 {
    let mut left = 0;
    let mut right = arr.len() - 1;
    
    while left <= right {
        let mid = (left + right) / 2;
        if arr[mid] == target {
            return mid;
        } else if arr[mid] < target {
            left = mid + 1;
        } else {
            right = mid - 1;
        }
    }
    
    return -1; // Not found
}
```

### Factorial (Recursive)
```palladium
fn factorial(n: i32) -> i32 {
    if n <= 1 {
        return 1;
    }
    return n * factorial(n - 1);
}
```

### Fibonacci (Iterative)
```palladium
fn fibonacci(n: i32) -> i32 {
    if n <= 1 {
        return n;
    }
    
    let mut a = 0;
    let mut b = 1;
    for i in 2..=n {
        let temp = a + b;
        a = b;
        b = temp;
    }
    
    return b;
}
```

## 🎨 The Ultimate Test: Splay Tree

```palladium
struct Node {
    key: i32,
    left: Option<Node>,
    right: Option<Node>,
}

fn splay(mut root: Node, key: i32) -> Node {
    // Complex rotation logic
    // This will prove Palladium is ready!
}

fn insert(root: Option<Node>, key: i32) -> Node {
    // Insert and splay
}

fn search(root: Node, key: i32) -> bool {
    // Search and splay to root
}
```

## 🛠️ Technical Challenges

1. **Loop Code Generation**: Proper labels and jumps
2. **Mutable Variables**: SSA form or direct mutation?
3. **Array Implementation**: Stack vs heap allocation
4. **Function Calls**: Calling convention design
5. **Type System**: Array types and inference

## 📊 Success Metrics

- [ ] All v0.2 tests still pass
- [ ] Can implement any algorithm from CLRS
- [ ] Splay tree implementation works correctly
- [ ] Performance within 5x of C
- [ ] Zero memory leaks
- [ ] Clear error messages for all new features

## 💡 Implementation Notes

1. **Start with While**: It's the fundamental loop construct
2. **Arrays Before Functions**: Easier to implement and test
3. **Keep It Simple**: Fixed-size arrays are fine for v0.3
4. **Test Everything**: Each feature needs comprehensive tests
5. **Document Semantics**: Be clear about evaluation order

## 🚨 Risk Mitigation

1. **Scope Creep**: Don't add features not in the plan
2. **Over-Engineering**: Simple implementations first
3. **Breaking Changes**: Maintain backward compatibility
4. **Performance**: Profile early and often
5. **Complexity**: Keep the language learnable

---

*"First, solve the problem. Then, write the code."* - John Johnson
*"In Palladium, we solve the problem AND prove it's correct!"* - AVP Team

**Let's make Palladium REAL! 🚀**