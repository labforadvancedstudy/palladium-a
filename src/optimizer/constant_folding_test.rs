#[cfg(test)]
mod tests {
    use super::super::*;
    use crate::errors::Span;

    fn create_test_program(body: Vec<Stmt>) -> Program {
        Program {
            imports: vec![],
            items: vec![Item::Function(Function {
                visibility: Visibility::Public,
                is_async: false,
                name: "test".to_string(),
                lifetime_params: vec![],
                type_params: vec![],
                const_params: vec![],
                params: vec![],
                return_type: Some(Type::Unit),
                body,
                span: Span::dummy(),
                effects: None,
            })],
        }
    }

    fn create_binary_expr(left: Expr, op: BinOp, right: Expr) -> Expr {
        Expr::Binary {
            left: Box::new(left),
            op,
            right: Box::new(right),
            span: Span::dummy(),
        }
    }

    fn create_unary_expr(op: UnaryOp, operand: Expr) -> Expr {
        Expr::Unary {
            op,
            operand: Box::new(operand),
            span: Span::dummy(),
        }
    }

    #[test]
    fn test_integer_arithmetic_folding() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test addition
        let mut expr = create_binary_expr(
            Expr::Integer(5),
            BinOp::Add,
            Expr::Integer(3),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Integer(8));

        // Test subtraction
        let mut expr = create_binary_expr(
            Expr::Integer(10),
            BinOp::Sub,
            Expr::Integer(4),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Integer(6));

        // Test multiplication
        let mut expr = create_binary_expr(
            Expr::Integer(7),
            BinOp::Mul,
            Expr::Integer(6),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Integer(42));

        // Test division
        let mut expr = create_binary_expr(
            Expr::Integer(20),
            BinOp::Div,
            Expr::Integer(4),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Integer(5));

        // Test modulo
        let mut expr = create_binary_expr(
            Expr::Integer(17),
            BinOp::Mod,
            Expr::Integer(5),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Integer(2));
    }

    #[test]
    fn test_division_by_zero_not_folded() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut expr = create_binary_expr(
            Expr::Integer(10),
            BinOp::Div,
            Expr::Integer(0),
        );
        // Should not fold division by zero
        assert!(!pass.optimize_expression(&mut expr).unwrap());
        
        // Verify expression is unchanged
        match expr {
            Expr::Binary { .. } => {} // Expected
            _ => panic!("Expression should remain a binary expression"),
        }
    }

    #[test]
    fn test_integer_comparison_folding() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test equality
        let mut expr = create_binary_expr(
            Expr::Integer(5),
            BinOp::Eq,
            Expr::Integer(5),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(true));

        // Test inequality
        let mut expr = create_binary_expr(
            Expr::Integer(5),
            BinOp::Ne,
            Expr::Integer(3),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(true));

        // Test less than
        let mut expr = create_binary_expr(
            Expr::Integer(3),
            BinOp::Lt,
            Expr::Integer(5),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(true));

        // Test greater than
        let mut expr = create_binary_expr(
            Expr::Integer(7),
            BinOp::Gt,
            Expr::Integer(2),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(true));

        // Test less or equal
        let mut expr = create_binary_expr(
            Expr::Integer(5),
            BinOp::Le,
            Expr::Integer(5),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(true));

        // Test greater or equal
        let mut expr = create_binary_expr(
            Expr::Integer(8),
            BinOp::Ge,
            Expr::Integer(3),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(true));
    }

    #[test]
    fn test_boolean_operations_folding() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test AND
        let mut expr = create_binary_expr(
            Expr::Bool(true),
            BinOp::And,
            Expr::Bool(false),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(false));

        // Test OR
        let mut expr = create_binary_expr(
            Expr::Bool(true),
            BinOp::Or,
            Expr::Bool(false),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(true));

        // Test boolean equality
        let mut expr = create_binary_expr(
            Expr::Bool(true),
            BinOp::Eq,
            Expr::Bool(true),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(true));

        // Test boolean inequality
        let mut expr = create_binary_expr(
            Expr::Bool(true),
            BinOp::Ne,
            Expr::Bool(false),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(true));
    }

    #[test]
    fn test_string_concatenation_folding() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut expr = create_binary_expr(
            Expr::String("Hello, ".to_string()),
            BinOp::Add,
            Expr::String("World!".to_string()),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::String("Hello, World!".to_string()));
    }

    #[test]
    fn test_unary_operations_folding() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test negation of integer
        let mut expr = create_unary_expr(
            UnaryOp::Neg,
            Expr::Integer(42),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Integer(-42));

        // Test NOT of boolean
        let mut expr = create_unary_expr(
            UnaryOp::Not,
            Expr::Bool(true),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Bool(false));
    }

    #[test]
    fn test_algebraic_simplifications() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test x + 0 => x
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Add,
            Expr::Integer(0),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));

        // Test 0 + x => x
        let mut expr = create_binary_expr(
            Expr::Integer(0),
            BinOp::Add,
            Expr::Ident("x".to_string()),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));

        // Test x - 0 => x
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Sub,
            Expr::Integer(0),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));

        // Test x * 0 => 0
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Mul,
            Expr::Integer(0),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Integer(0));

        // Test 0 * x => 0
        let mut expr = create_binary_expr(
            Expr::Integer(0),
            BinOp::Mul,
            Expr::Ident("x".to_string()),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Integer(0));

        // Test x * 1 => x
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Mul,
            Expr::Integer(1),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));

        // Test 1 * x => x
        let mut expr = create_binary_expr(
            Expr::Integer(1),
            BinOp::Mul,
            Expr::Ident("x".to_string()),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));

        // Test x / 1 => x
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Div,
            Expr::Integer(1),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));
    }

    #[test]
    fn test_nested_expression_folding() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test (2 + 3) * (4 + 1) => 5 * 5 => 25
        let mut expr = create_binary_expr(
            create_binary_expr(
                Expr::Integer(2),
                BinOp::Add,
                Expr::Integer(3),
            ),
            BinOp::Mul,
            create_binary_expr(
                Expr::Integer(4),
                BinOp::Add,
                Expr::Integer(1),
            ),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Integer(25));
    }

    #[test]
    fn test_folding_in_statements() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test folding in let statement
        let mut stmt = Stmt::Let {
            name: "x".to_string(),
            ty: None,
            value: create_binary_expr(
                Expr::Integer(10),
                BinOp::Add,
                Expr::Integer(20),
            ),
            mutable: false,
            span: Span::dummy(),
        };
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::Let { value, .. } => {
                assert_eq!(value, Expr::Integer(30));
            }
            _ => panic!("Expected Let statement"),
        }
    }

    #[test]
    fn test_folding_in_if_condition() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut stmt = Stmt::If {
            condition: create_binary_expr(
                Expr::Integer(5),
                BinOp::Gt,
                Expr::Integer(3),
            ),
            then_branch: vec![],
            else_branch: None,
            span: Span::dummy(),
        };
        
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::If { condition, .. } => {
                assert_eq!(condition, Expr::Bool(true));
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_folding_in_function_call_args() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut expr = Expr::Call {
            func: Box::new(Expr::Ident("print".to_string())),
            args: vec![
                create_binary_expr(
                    Expr::Integer(2),
                    BinOp::Mul,
                    Expr::Integer(21),
                ),
            ],
            span: Span::dummy(),
        };
        
        pass.optimize_expression(&mut expr).unwrap();
        
        match expr {
            Expr::Call { args, .. } => {
                assert_eq!(args[0], Expr::Integer(42));
            }
            _ => panic!("Expected Call expression"),
        }
    }

    #[test]
    fn test_folding_in_array_literal() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut expr = Expr::ArrayLiteral {
            elements: vec![
                create_binary_expr(
                    Expr::Integer(1),
                    BinOp::Add,
                    Expr::Integer(2),
                ),
                create_binary_expr(
                    Expr::Integer(3),
                    BinOp::Mul,
                    Expr::Integer(4),
                ),
            ],
            span: Span::dummy(),
        };
        
        pass.optimize_expression(&mut expr).unwrap();
        
        match expr {
            Expr::ArrayLiteral { elements, .. } => {
                assert_eq!(elements[0], Expr::Integer(3));
                assert_eq!(elements[1], Expr::Integer(12));
            }
            _ => panic!("Expected ArrayLiteral expression"),
        }
    }

    #[test]
    fn test_program_optimization() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut program = create_test_program(vec![
            Stmt::Let {
                name: "x".to_string(),
                ty: None,
                value: create_binary_expr(
                    Expr::Integer(10),
                    BinOp::Add,
                    Expr::Integer(20),
                ),
                mutable: false,
                span: Span::dummy(),
            },
            Stmt::Return(Some(create_binary_expr(
                Expr::Integer(5),
                BinOp::Mul,
                Expr::Integer(6),
            ))),
        ]);
        
        assert!(pass.optimize_program(&mut program).unwrap());
        
        match &program.items[0] {
            Item::Function(func) => {
                match &func.body[0] {
                    Stmt::Let { value, .. } => {
                        assert_eq!(*value, Expr::Integer(30));
                    }
                    _ => panic!("Expected Let statement"),
                }
                match &func.body[1] {
                    Stmt::Return(Some(expr)) => {
                        assert_eq!(*expr, Expr::Integer(30));
                    }
                    _ => panic!("Expected Return statement"),
                }
            }
            _ => panic!("Expected Function item"),
        }
    }

    #[test]
    fn test_mixed_type_operations_not_folded() {
        let mut pass = ConstantFoldingPass::new();
        
        // Integer + String should not be folded
        let mut expr = create_binary_expr(
            Expr::Integer(5),
            BinOp::Add,
            Expr::String("hello".to_string()),
        );
        assert!(!pass.optimize_expression(&mut expr).unwrap());
        
        // Verify expression is unchanged
        match expr {
            Expr::Binary { .. } => {} // Expected
            _ => panic!("Expression should remain a binary expression"),
        }
    }

    #[test]
    fn test_multiple_passes_reach_fixed_point() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test that ((1 + 2) + (3 + 4)) + 5 is fully folded
        let mut expr = create_binary_expr(
            create_binary_expr(
                create_binary_expr(
                    Expr::Integer(1),
                    BinOp::Add,
                    Expr::Integer(2),
                ),
                BinOp::Add,
                create_binary_expr(
                    Expr::Integer(3),
                    BinOp::Add,
                    Expr::Integer(4),
                ),
            ),
            BinOp::Add,
            Expr::Integer(5),
        );
        
        // First pass might not fold everything
        pass.optimize_expression(&mut expr).unwrap();
        
        // But after enough passes, should reach fixed point
        let mut optimizer = Optimizer::new();
        let mut program = create_test_program(vec![Stmt::Expr(expr)]);
        optimizer.optimize(&mut program).unwrap();
        
        match &program.items[0] {
            Item::Function(func) => {
                match &func.body[0] {
                    Stmt::Expr(expr) => {
                        assert_eq!(*expr, Expr::Integer(15));
                    }
                    _ => panic!("Expected Expr statement"),
                }
            }
            _ => panic!("Expected Function item"),
        }
    }

    #[test]
    fn test_folding_preserves_semantics() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test overflow behavior is preserved
        let mut expr = create_binary_expr(
            Expr::Integer(i64::MAX),
            BinOp::Add,
            Expr::Integer(1),
        );
        pass.optimize_expression(&mut expr).unwrap();
        assert_eq!(expr, Expr::Integer(i64::MIN)); // Wrapping overflow
    }

    #[test]
    fn test_nested_unary_operations() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test -(-(-42)) => -42
        let mut expr = create_unary_expr(
            UnaryOp::Neg,
            create_unary_expr(
                UnaryOp::Neg,
                create_unary_expr(
                    UnaryOp::Neg,
                    Expr::Integer(42),
                ),
            ),
        );
        
        pass.optimize_expression(&mut expr).unwrap();
        assert_eq!(expr, Expr::Integer(-42));
    }

    #[test]
    fn test_comparison_chain_folding() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test (5 > 3) && (10 < 20) => true && true => true
        let mut expr = create_binary_expr(
            create_binary_expr(
                Expr::Integer(5),
                BinOp::Gt,
                Expr::Integer(3),
            ),
            BinOp::And,
            create_binary_expr(
                Expr::Integer(10),
                BinOp::Lt,
                Expr::Integer(20),
            ),
        );
        
        pass.optimize_expression(&mut expr).unwrap();
        assert_eq!(expr, Expr::Bool(true));
    }

    #[test]
    fn test_field_access_optimization() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut expr = Expr::FieldAccess {
            object: Box::new(create_binary_expr(
                Expr::Integer(1),
                BinOp::Add,
                Expr::Integer(2),
            )),
            field: "value".to_string(),
            span: Span::dummy(),
        };
        
        pass.optimize_expression(&mut expr).unwrap();
        
        match expr {
            Expr::FieldAccess { object, field, .. } => {
                assert_eq!(object.as_ref(), &Expr::Integer(3));
                assert_eq!(field, "value");
            }
            _ => panic!("Expected FieldAccess expression"),
        }
    }

    #[test]
    fn test_while_loop_condition_folding() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut stmt = Stmt::While {
            condition: create_binary_expr(
                Expr::Integer(10),
                BinOp::Gt,
                Expr::Integer(5),
            ),
            body: vec![Stmt::Expr(Expr::Integer(1))],
            span: Span::dummy(),
        };
        
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::While { condition, .. } => {
                assert_eq!(condition, Expr::Bool(true));
            }
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_assign_statement_optimization() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut stmt = Stmt::Assign {
            target: AssignTarget::Ident("x".to_string()),
            value: create_binary_expr(
                Expr::Integer(100),
                BinOp::Div,
                Expr::Integer(10),
            ),
            span: Span::dummy(),
        };
        
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::Assign { value, .. } => {
                assert_eq!(value, Expr::Integer(10));
            }
            _ => panic!("Expected Assign statement"),
        }
    }

    #[test]
    fn test_array_index_optimization() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut expr = Expr::Index {
            array: Box::new(Expr::Ident("arr".to_string())),
            index: Box::new(create_binary_expr(
                Expr::Integer(2),
                BinOp::Mul,
                Expr::Integer(3),
            )),
            span: Span::dummy(),
        };
        
        pass.optimize_expression(&mut expr).unwrap();
        
        match expr {
            Expr::Index { index, .. } => {
                assert_eq!(index.as_ref(), &Expr::Integer(6));
            }
            _ => panic!("Expected Index expression"),
        }
    }

    #[test]
    fn test_complex_arithmetic_folding() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test ((100 / 5) + (3 * 7)) - (2 + 8) => (20 + 21) - 10 => 31
        let mut expr = create_binary_expr(
            create_binary_expr(
                create_binary_expr(
                    Expr::Integer(100),
                    BinOp::Div,
                    Expr::Integer(5),
                ),
                BinOp::Add,
                create_binary_expr(
                    Expr::Integer(3),
                    BinOp::Mul,
                    Expr::Integer(7),
                ),
            ),
            BinOp::Sub,
            create_binary_expr(
                Expr::Integer(2),
                BinOp::Add,
                Expr::Integer(8),
            ),
        );
        
        pass.optimize_expression(&mut expr).unwrap();
        assert_eq!(expr, Expr::Integer(31));
    }

    #[test]
    fn test_modulo_by_zero_not_folded() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut expr = create_binary_expr(
            Expr::Integer(10),
            BinOp::Mod,
            Expr::Integer(0),
        );
        
        // Should not fold modulo by zero
        assert!(!pass.optimize_expression(&mut expr).unwrap());
        
        match expr {
            Expr::Binary { .. } => {} // Expected
            _ => panic!("Expression should remain a binary expression"),
        }
    }

    #[test]
    fn test_short_circuit_evaluation() {
        let mut pass = ConstantFoldingPass::new();
        
        // Test false && <anything> => false (but we still optimize the right side)
        let mut expr = create_binary_expr(
            Expr::Bool(false),
            BinOp::And,
            create_binary_expr(
                Expr::Integer(1),
                BinOp::Add,
                Expr::Integer(2),
            ),
        );
        
        pass.optimize_expression(&mut expr).unwrap();
        // Should fold to false && 3 => false
        assert_eq!(expr, Expr::Bool(false));
        
        // Test true || <anything> => true (but we still optimize the right side)
        let mut expr = create_binary_expr(
            Expr::Bool(true),
            BinOp::Or,
            create_binary_expr(
                Expr::Integer(5),
                BinOp::Mul,
                Expr::Integer(6),
            ),
        );
        
        pass.optimize_expression(&mut expr).unwrap();
        // Should fold to true || 30 => true
        assert_eq!(expr, Expr::Bool(true));
    }

    #[test]
    fn test_empty_array_literal() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut expr = Expr::ArrayLiteral {
            elements: vec![],
            span: Span::dummy(),
        };
        
        // Empty array should not change
        assert!(!pass.optimize_expression(&mut expr).unwrap());
        
        match expr {
            Expr::ArrayLiteral { elements, .. } => {
                assert!(elements.is_empty());
            }
            _ => panic!("Expected ArrayLiteral expression"),
        }
    }

    #[test]
    fn test_nested_if_conditions() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut stmt = Stmt::If {
            condition: create_binary_expr(
                Expr::Integer(10),
                BinOp::Eq,
                Expr::Integer(10),
            ),
            then_branch: vec![
                Stmt::If {
                    condition: create_binary_expr(
                        Expr::Bool(true),
                        BinOp::And,
                        Expr::Bool(false),
                    ),
                    then_branch: vec![],
                    else_branch: None,
                    span: Span::dummy(),
                },
            ],
            else_branch: Some(vec![
                Stmt::Expr(create_binary_expr(
                    Expr::Integer(1),
                    BinOp::Add,
                    Expr::Integer(1),
                )),
            ]),
            span: Span::dummy(),
        };
        
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::If { condition, then_branch, else_branch, .. } => {
                assert_eq!(condition, Expr::Bool(true));
                match &then_branch[0] {
                    Stmt::If { condition: inner_cond, .. } => {
                        assert_eq!(*inner_cond, Expr::Bool(false));
                    }
                    _ => panic!("Expected nested If statement"),
                }
                match &else_branch.as_ref().unwrap()[0] {
                    Stmt::Expr(expr) => {
                        assert_eq!(*expr, Expr::Integer(2));
                    }
                    _ => panic!("Expected Expr statement"),
                }
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_for_loop_optimization() {
        let mut pass = ConstantFoldingPass::new();
        
        let mut stmt = Stmt::For {
            var: "i".to_string(),
            iter: create_binary_expr(
                Expr::Integer(1),
                BinOp::Add,
                Expr::Integer(2),
            ),
            body: vec![],
            span: Span::dummy(),
        };
        
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::For { iter, .. } => {
                assert_eq!(iter, Expr::Integer(3));
            }
            _ => panic!("Expected For statement"),
        }
    }

    #[test]
    fn test_no_changes_tracking() {
        let mut pass = ConstantFoldingPass::new();
        
        // Expression that cannot be folded
        let mut expr = Expr::Ident("x".to_string());
        assert!(!pass.optimize_expression(&mut expr).unwrap());
        
        // Verify no changes were made
        // Note: changes_made is private, so we can't directly access it
    }

    #[test]
    fn test_string_non_add_operations() {
        let mut pass = ConstantFoldingPass::new();
        
        // String comparison should not be folded
        let mut expr = create_binary_expr(
            Expr::String("hello".to_string()),
            BinOp::Eq,
            Expr::String("world".to_string()),
        );
        assert!(!pass.optimize_expression(&mut expr).unwrap());
        
        match expr {
            Expr::Binary { .. } => {} // Expected
            _ => panic!("Expression should remain a binary expression"),
        }
    }

    #[test]
    fn test_optimizer_with_logging() {
        let mut optimizer = Optimizer::new().with_logging();
        
        let mut program = create_test_program(vec![
            Stmt::Expr(create_binary_expr(
                Expr::Integer(10),
                BinOp::Add,
                Expr::Integer(20),
            )),
        ]);
        
        // Should not panic with logging enabled
        optimizer.optimize(&mut program).unwrap();
    }
}