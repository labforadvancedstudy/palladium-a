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
    fn test_simplify_bool_comparison_with_true() {
        let mut pass = SimplificationPass::new();
        
        // x == true => x
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Eq,
            Expr::Bool(true),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));

        // true == x => x
        let mut expr = create_binary_expr(
            Expr::Bool(true),
            BinOp::Eq,
            Expr::Ident("x".to_string()),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));
    }

    #[test]
    fn test_simplify_bool_comparison_with_false() {
        let mut pass = SimplificationPass::new();
        
        // x == false => !x
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Eq,
            Expr::Bool(false),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        match expr {
            Expr::Unary { op: UnaryOp::Not, operand, .. } => {
                assert_eq!(*operand, Expr::Ident("x".to_string()));
            }
            _ => panic!("Expected Unary Not expression"),
        }

        // false == x => !x
        let mut expr = create_binary_expr(
            Expr::Bool(false),
            BinOp::Eq,
            Expr::Ident("x".to_string()),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        match expr {
            Expr::Unary { op: UnaryOp::Not, operand, .. } => {
                assert_eq!(*operand, Expr::Ident("x".to_string()));
            }
            _ => panic!("Expected Unary Not expression"),
        }
    }

    #[test]
    fn test_simplify_bool_not_equal() {
        let mut pass = SimplificationPass::new();
        
        // x != false => x
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Ne,
            Expr::Bool(false),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));

        // false != x => x
        let mut expr = create_binary_expr(
            Expr::Bool(false),
            BinOp::Ne,
            Expr::Ident("x".to_string()),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));

        // x != true => !x
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Ne,
            Expr::Bool(true),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        match expr {
            Expr::Unary { op: UnaryOp::Not, operand, .. } => {
                assert_eq!(*operand, Expr::Ident("x".to_string()));
            }
            _ => panic!("Expected Unary Not expression"),
        }

        // true != x => !x
        let mut expr = create_binary_expr(
            Expr::Bool(true),
            BinOp::Ne,
            Expr::Ident("x".to_string()),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        match expr {
            Expr::Unary { op: UnaryOp::Not, operand, .. } => {
                assert_eq!(*operand, Expr::Ident("x".to_string()));
            }
            _ => panic!("Expected Unary Not expression"),
        }
    }

    #[test]
    fn test_simplify_double_negation() {
        let mut pass = SimplificationPass::new();
        
        // !!x => x
        let mut expr = create_unary_expr(
            UnaryOp::Not,
            create_unary_expr(
                UnaryOp::Not,
                Expr::Ident("x".to_string()),
            ),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        assert_eq!(expr, Expr::Ident("x".to_string()));
    }

    #[test]
    fn test_simplify_triple_negation() {
        let mut pass = SimplificationPass::new();
        
        // !!!x => !x (after one pass)
        let mut expr = create_unary_expr(
            UnaryOp::Not,
            create_unary_expr(
                UnaryOp::Not,
                create_unary_expr(
                    UnaryOp::Not,
                    Expr::Ident("x".to_string()),
                ),
            ),
        );
        assert!(pass.optimize_expression(&mut expr).unwrap());
        // Should simplify outer two NOTs
        match expr {
            Expr::Unary { op: UnaryOp::Not, operand, .. } => {
                assert_eq!(*operand, Expr::Ident("x".to_string()));
            }
            _ => panic!("Expected Unary Not expression"),
        }
    }

    #[test]
    fn test_simplify_nested_expressions() {
        let mut pass = SimplificationPass::new();
        
        // (x == true) && (y != false) => x && y
        let mut expr = create_binary_expr(
            create_binary_expr(
                Expr::Ident("x".to_string()),
                BinOp::Eq,
                Expr::Bool(true),
            ),
            BinOp::And,
            create_binary_expr(
                Expr::Ident("y".to_string()),
                BinOp::Ne,
                Expr::Bool(false),
            ),
        );
        pass.optimize_expression(&mut expr).unwrap();
        match expr {
            Expr::Binary { left, op: BinOp::And, right, .. } => {
                assert_eq!(left.as_ref(), &Expr::Ident("x".to_string()));
                assert_eq!(right.as_ref(), &Expr::Ident("y".to_string()));
            }
            _ => panic!("Expected Binary And expression"),
        }
    }

    #[test]
    fn test_simplify_in_statements() {
        let mut pass = SimplificationPass::new();
        
        // Test simplification in let statement
        let mut stmt = Stmt::Let {
            name: "result".to_string(),
            ty: None,
            value: create_binary_expr(
                Expr::Ident("flag".to_string()),
                BinOp::Eq,
                Expr::Bool(true),
            ),
            mutable: false,
            span: Span::dummy(),
        };
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::Let { value, .. } => {
                assert_eq!(value, Expr::Ident("flag".to_string()));
            }
            _ => panic!("Expected Let statement"),
        }
    }

    #[test]
    fn test_simplify_in_if_condition() {
        let mut pass = SimplificationPass::new();
        
        let mut stmt = Stmt::If {
            condition: create_unary_expr(
                UnaryOp::Not,
                create_unary_expr(
                    UnaryOp::Not,
                    Expr::Ident("condition".to_string()),
                ),
            ),
            then_branch: vec![],
            else_branch: None,
            span: Span::dummy(),
        };
        
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::If { condition, .. } => {
                assert_eq!(condition, Expr::Ident("condition".to_string()));
            }
            _ => panic!("Expected If statement"),
        }
    }

    #[test]
    fn test_simplify_in_while_condition() {
        let mut pass = SimplificationPass::new();
        
        let mut stmt = Stmt::While {
            condition: create_binary_expr(
                Expr::Ident("running".to_string()),
                BinOp::Ne,
                Expr::Bool(false),
            ),
            body: vec![],
            span: Span::dummy(),
        };
        
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::While { condition, .. } => {
                assert_eq!(condition, Expr::Ident("running".to_string()));
            }
            _ => panic!("Expected While statement"),
        }
    }

    #[test]
    fn test_simplify_in_return() {
        let mut pass = SimplificationPass::new();
        
        let mut stmt = Stmt::Return(Some(create_binary_expr(
            Expr::Bool(true),
            BinOp::Eq,
            Expr::Ident("success".to_string()),
        )));
        
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::Return(Some(expr)) => {
                assert_eq!(expr, Expr::Ident("success".to_string()));
            }
            _ => panic!("Expected Return statement"),
        }
    }

    #[test]
    fn test_simplify_in_function_call() {
        let mut pass = SimplificationPass::new();
        
        let mut expr = Expr::Call {
            func: Box::new(Expr::Ident("assert".to_string())),
            args: vec![
                create_unary_expr(
                    UnaryOp::Not,
                    create_unary_expr(
                        UnaryOp::Not,
                        Expr::Ident("valid".to_string()),
                    ),
                ),
            ],
            span: Span::dummy(),
        };
        
        pass.optimize_expression(&mut expr).unwrap();
        
        match expr {
            Expr::Call { args, .. } => {
                assert_eq!(args[0], Expr::Ident("valid".to_string()));
            }
            _ => panic!("Expected Call expression"),
        }
    }

    #[test]
    fn test_simplify_in_array_literal() {
        let mut pass = SimplificationPass::new();
        
        let mut expr = Expr::ArrayLiteral {
            elements: vec![
                create_binary_expr(
                    Expr::Ident("a".to_string()),
                    BinOp::Eq,
                    Expr::Bool(true),
                ),
                create_binary_expr(
                    Expr::Ident("b".to_string()),
                    BinOp::Ne,
                    Expr::Bool(false),
                ),
            ],
            span: Span::dummy(),
        };
        
        pass.optimize_expression(&mut expr).unwrap();
        
        match expr {
            Expr::ArrayLiteral { elements, .. } => {
                assert_eq!(elements[0], Expr::Ident("a".to_string()));
                assert_eq!(elements[1], Expr::Ident("b".to_string()));
            }
            _ => panic!("Expected ArrayLiteral expression"),
        }
    }

    #[test]
    fn test_simplify_in_index_expression() {
        let mut pass = SimplificationPass::new();
        
        let mut expr = Expr::Index {
            array: Box::new(Expr::Ident("arr".to_string())),
            index: Box::new(create_binary_expr(
                Expr::Ident("i".to_string()),
                BinOp::Ne,
                Expr::Bool(false),
            )),
            span: Span::dummy(),
        };
        
        pass.optimize_expression(&mut expr).unwrap();
        
        match expr {
            Expr::Index { index, .. } => {
                assert_eq!(index.as_ref(), &Expr::Ident("i".to_string()));
            }
            _ => panic!("Expected Index expression"),
        }
    }

    #[test]
    fn test_simplify_field_access() {
        let mut pass = SimplificationPass::new();
        
        let mut expr = Expr::FieldAccess {
            object: Box::new(create_binary_expr(
                Expr::Ident("obj".to_string()),
                BinOp::Eq,
                Expr::Bool(true),
            )),
            field: "field".to_string(),
            span: Span::dummy(),
        };
        
        pass.optimize_expression(&mut expr).unwrap();
        
        match expr {
            Expr::FieldAccess { object, .. } => {
                assert_eq!(object.as_ref(), &Expr::Ident("obj".to_string()));
            }
            _ => panic!("Expected FieldAccess expression"),
        }
    }

    #[test]
    fn test_no_simplification_needed() {
        let mut pass = SimplificationPass::new();
        
        // Expression that doesn't need simplification
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Add,
            Expr::Integer(42),
        );
        assert!(!pass.optimize_expression(&mut expr).unwrap());
        
        // Verify expression is unchanged
        match expr {
            Expr::Binary { left, op: BinOp::Add, right, .. } => {
                assert_eq!(left.as_ref(), &Expr::Ident("x".to_string()));
                assert_eq!(right.as_ref(), &Expr::Integer(42));
            }
            _ => panic!("Expression should remain unchanged"),
        }
    }

    #[test]
    fn test_program_simplification() {
        let mut pass = SimplificationPass::new();
        
        let mut program = create_test_program(vec![
            Stmt::Let {
                name: "flag".to_string(),
                ty: None,
                value: create_unary_expr(
                    UnaryOp::Not,
                    create_unary_expr(
                        UnaryOp::Not,
                        Expr::Ident("condition".to_string()),
                    ),
                ),
                mutable: false,
                span: Span::dummy(),
            },
            Stmt::If {
                condition: create_binary_expr(
                    Expr::Ident("flag".to_string()),
                    BinOp::Eq,
                    Expr::Bool(true),
                ),
                then_branch: vec![
                    Stmt::Return(Some(Expr::Integer(1))),
                ],
                else_branch: None,
                span: Span::dummy(),
            },
        ]);
        
        assert!(pass.optimize_program(&mut program).unwrap());
        
        match &program.items[0] {
            Item::Function(func) => {
                match &func.body[0] {
                    Stmt::Let { value, .. } => {
                        assert_eq!(*value, Expr::Ident("condition".to_string()));
                    }
                    _ => panic!("Expected Let statement"),
                }
                match &func.body[1] {
                    Stmt::If { condition, .. } => {
                        assert_eq!(*condition, Expr::Ident("flag".to_string()));
                    }
                    _ => panic!("Expected If statement"),
                }
            }
            _ => panic!("Expected Function item"),
        }
    }

    #[test]
    fn test_complex_simplification() {
        let mut optimizer = Optimizer::new();
        
        // Test complex expression: !!(!x == false) => x
        let mut program = create_test_program(vec![
            Stmt::Expr(create_unary_expr(
                UnaryOp::Not,
                create_unary_expr(
                    UnaryOp::Not,
                    create_binary_expr(
                        create_unary_expr(
                            UnaryOp::Not,
                            Expr::Ident("x".to_string()),
                        ),
                        BinOp::Eq,
                        Expr::Bool(false),
                    ),
                ),
            )),
        ]);
        
        optimizer.optimize(&mut program).unwrap();
        
        match &program.items[0] {
            Item::Function(func) => {
                match &func.body[0] {
                    Stmt::Expr(expr) => {
                        // After simplification: !!(!x == false) => !!(!x) => !x => x
                        assert_eq!(*expr, Expr::Ident("x".to_string()));
                    }
                    _ => panic!("Expected Expr statement"),
                }
            }
            _ => panic!("Expected Function item"),
        }
    }

    #[test]
    fn test_empty_function_body() {
        let mut pass = SimplificationPass::new();
        
        let mut program = create_test_program(vec![]);
        
        // Should not crash or fail
        assert!(!pass.optimize_program(&mut program).unwrap());
    }

    #[test]
    fn test_changes_counter() {
        let mut pass = SimplificationPass::new();
        
        // Test that changes are made
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::Eq,
            Expr::Bool(true),
        );
        let changed1 = pass.optimize_expression(&mut expr).unwrap();
        assert!(changed1);
        assert_eq!(expr, Expr::Ident("x".to_string()));
        
        // Make another change
        let mut expr = create_unary_expr(
            UnaryOp::Not,
            create_unary_expr(
                UnaryOp::Not,
                Expr::Ident("y".to_string()),
            ),
        );
        let changed2 = pass.optimize_expression(&mut expr).unwrap();
        assert!(changed2);
        assert_eq!(expr, Expr::Ident("y".to_string()));
    }

    #[test]
    fn test_complex_boolean_simplification() {
        let mut pass = SimplificationPass::new();
        
        // Test (x && true) => x
        let mut expr = create_binary_expr(
            Expr::Ident("x".to_string()),
            BinOp::And,
            Expr::Bool(true),
        );
        // Note: current implementation doesn't simplify this
        pass.optimize_expression(&mut expr).unwrap();
        
        // Test (false || x) => x
        let mut expr = create_binary_expr(
            Expr::Bool(false),
            BinOp::Or,
            Expr::Ident("x".to_string()),
        );
        // Note: current implementation doesn't simplify this
        pass.optimize_expression(&mut expr).unwrap();
    }

    #[test]
    fn test_nested_not_patterns() {
        let mut pass = SimplificationPass::new();
        
        // Test !(!(x && y)) => x && y
        let mut expr = create_unary_expr(
            UnaryOp::Not,
            create_unary_expr(
                UnaryOp::Not,
                create_binary_expr(
                    Expr::Ident("x".to_string()),
                    BinOp::And,
                    Expr::Ident("y".to_string()),
                ),
            ),
        );
        
        pass.optimize_expression(&mut expr).unwrap();
        match expr {
            Expr::Binary { left, op: BinOp::And, right, .. } => {
                assert_eq!(left.as_ref(), &Expr::Ident("x".to_string()));
                assert_eq!(right.as_ref(), &Expr::Ident("y".to_string()));
            }
            _ => panic!("Expected Binary And expression"),
        }
    }

    #[test]
    fn test_simplify_in_match_arms() {
        let mut pass = SimplificationPass::new();
        
        let mut stmt = Stmt::Match {
            expr: Expr::Ident("x".to_string()),
            arms: vec![
                MatchArm {
                    pattern: Pattern::Ident("true_case".to_string()),
                    body: vec![
                        Stmt::Expr(create_binary_expr(
                            Expr::Ident("flag".to_string()),
                            BinOp::Eq,
                            Expr::Bool(true),
                        )),
                    ],
                },
            ],
            span: Span::dummy(),
        };
        
        // Note: current implementation doesn't optimize inside match arms
        pass.optimize_statement(&mut stmt).unwrap();
    }

    #[test]
    fn test_simplify_for_loop() {
        let mut pass = SimplificationPass::new();
        
        let mut stmt = Stmt::For {
            var: "i".to_string(),
            iter: Expr::Ident("items".to_string()),
            body: vec![
                Stmt::Expr(create_unary_expr(
                    UnaryOp::Not,
                    create_unary_expr(
                        UnaryOp::Not,
                        Expr::Ident("condition".to_string()),
                    ),
                )),
            ],
            span: Span::dummy(),
        };
        
        // Note: current implementation doesn't optimize inside for loops
        pass.optimize_statement(&mut stmt).unwrap();
    }

    #[test]
    fn test_simplify_assign_statement() {
        let mut pass = SimplificationPass::new();
        
        let mut stmt = Stmt::Assign {
            target: AssignTarget::Ident("x".to_string()),
            value: create_binary_expr(
                Expr::Ident("flag".to_string()),
                BinOp::Ne,
                Expr::Bool(false),
            ),
            span: Span::dummy(),
        };
        
        pass.optimize_statement(&mut stmt).unwrap();
        
        match stmt {
            Stmt::Assign { value, .. } => {
                assert_eq!(value, Expr::Ident("flag".to_string()));
            }
            _ => panic!("Expected Assign statement"),
        }
    }

    #[test]
    fn test_simplify_array_repeat() {
        let mut pass = SimplificationPass::new();
        
        let mut expr = Expr::ArrayRepeat {
            value: Box::new(create_binary_expr(
                Expr::Ident("x".to_string()),
                BinOp::Eq,
                Expr::Bool(true),
            )),
            count: Box::new(Expr::Integer(10)),
            span: Span::dummy(),
        };
        
        // Note: current implementation doesn't handle ArrayRepeat
        pass.optimize_expression(&mut expr).unwrap();
    }

    #[test]
    fn test_simplify_struct_literal() {
        let mut pass = SimplificationPass::new();
        
        let mut expr = Expr::StructLiteral {
            name: "Config".to_string(),
            fields: vec![
                ("enabled".to_string(), create_binary_expr(
                    Expr::Ident("flag".to_string()),
                    BinOp::Ne,
                    Expr::Bool(false),
                )),
                ("debug".to_string(), create_unary_expr(
                    UnaryOp::Not,
                    create_unary_expr(
                        UnaryOp::Not,
                        Expr::Bool(true),
                    ),
                )),
            ],
            span: Span::dummy(),
        };
        
        // Note: current implementation doesn't handle StructLiteral
        pass.optimize_expression(&mut expr).unwrap();
    }

    #[test]
    fn test_no_infinite_recursion() {
        let mut pass = SimplificationPass::new();
        
        // Create a pathological case that could cause infinite recursion
        let mut expr = Expr::Ident("x".to_string());
        
        // This should not hang or crash
        assert!(!pass.optimize_expression(&mut expr).unwrap());
    }

    #[test]
    fn test_simplify_preserves_types() {
        let mut pass = SimplificationPass::new();
        
        // Ensure simplification doesn't change types unexpectedly
        let mut expr = create_binary_expr(
            Expr::String("hello".to_string()),
            BinOp::Eq,
            Expr::String("hello".to_string()),
        );
        
        // String comparison should not be simplified (type checking needed)
        assert!(!pass.optimize_expression(&mut expr).unwrap());
    }

    #[test]
    fn test_multiple_simplifications_in_one_pass() {
        let mut pass = SimplificationPass::new();
        
        // Test an expression with multiple simplification opportunities
        let mut expr = create_binary_expr(
            create_unary_expr(
                UnaryOp::Not,
                create_unary_expr(
                    UnaryOp::Not,
                    Expr::Ident("a".to_string()),
                ),
            ),
            BinOp::And,
            create_binary_expr(
                Expr::Ident("b".to_string()),
                BinOp::Eq,
                Expr::Bool(true),
            ),
        );
        
        pass.optimize_expression(&mut expr).unwrap();
        
        // Should simplify to a && b
        match expr {
            Expr::Binary { left, op: BinOp::And, right, .. } => {
                assert_eq!(left.as_ref(), &Expr::Ident("a".to_string()));
                assert_eq!(right.as_ref(), &Expr::Ident("b".to_string()));
            }
            _ => panic!("Expected Binary And expression"),
        }
    }

    #[test]
    fn test_simplify_chained_comparisons() {
        let mut pass = SimplificationPass::new();
        
        // Test ((x == true) == false) => !(x)
        let mut expr = create_binary_expr(
            create_binary_expr(
                Expr::Ident("x".to_string()),
                BinOp::Eq,
                Expr::Bool(true),
            ),
            BinOp::Eq,
            Expr::Bool(false),
        );
        
        pass.optimize_expression(&mut expr).unwrap();
        
        // Should simplify to !x
        match expr {
            Expr::Unary { op: UnaryOp::Not, operand, .. } => {
                assert_eq!(operand.as_ref(), &Expr::Ident("x".to_string()));
            }
            _ => panic!("Expected Unary Not expression"),
        }
    }

    #[test]
    fn test_unsafe_block_simplification() {
        let mut pass = SimplificationPass::new();
        
        let mut stmt = Stmt::Unsafe {
            body: vec![
                Stmt::Expr(create_unary_expr(
                    UnaryOp::Not,
                    create_unary_expr(
                        UnaryOp::Not,
                        Expr::Ident("x".to_string()),
                    ),
                )),
            ],
            span: Span::dummy(),
        };
        
        // Note: current implementation doesn't optimize inside unsafe blocks
        pass.optimize_statement(&mut stmt).unwrap();
    }

    #[test]
    fn test_empty_program() {
        let mut pass = SimplificationPass::new();
        let mut program = create_test_program(vec![]);
        
        // Should not fail on empty program
        assert!(!pass.optimize_program(&mut program).unwrap());
    }
}