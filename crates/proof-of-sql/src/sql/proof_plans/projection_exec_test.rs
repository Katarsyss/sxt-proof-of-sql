use super::{test_utility::*, DynProofPlan, ProjectionExec};
use crate::{
    base::{
        database::{
            owned_table_utility::*, Column, ColumnField, ColumnRef, ColumnType, OwnedTable,
            OwnedTableTestAccessor, TableRef, TestAccessor,
        },
        map::{IndexMap, IndexSet},
        math::decimal::Precision,
        scalar::Curve25519Scalar,
    },
    sql::{
        proof::{
            exercise_verification, FirstRoundBuilder, ProofPlan, ProvableQueryResult,
            ProverEvaluate, VerifiableQueryResult,
        },
        proof_exprs::{test_utility::*, ColumnExpr, DynProofExpr, TableExpr},
    },
};
use blitzar::proof::InnerProductProof;
use bumpalo::Bump;
use curve25519_dalek::RistrettoPoint;
use proof_of_sql_parser::{Identifier, ResourceId};

#[test]
fn we_can_correctly_fetch_the_query_result_schema() {
    let table_ref = TableRef::new(ResourceId::try_new("sxt", "sxt_tab").unwrap());
    let a = Identifier::try_new("a").unwrap();
    let b = Identifier::try_new("b").unwrap();
    let provable_ast = ProjectionExec::<RistrettoPoint>::new(
        vec![
            aliased_plan(
                DynProofExpr::Column(ColumnExpr::new(ColumnRef::new(
                    table_ref,
                    a,
                    ColumnType::BigInt,
                ))),
                "a",
            ),
            aliased_plan(
                DynProofExpr::Column(ColumnExpr::new(ColumnRef::new(
                    table_ref,
                    b,
                    ColumnType::BigInt,
                ))),
                "b",
            ),
        ],
        TableExpr { table_ref },
    );
    let column_fields: Vec<ColumnField> = provable_ast.get_column_result_fields();
    assert_eq!(
        column_fields,
        vec![
            ColumnField::new("a".parse().unwrap(), ColumnType::BigInt),
            ColumnField::new("b".parse().unwrap(), ColumnType::BigInt),
        ]
    );
}

#[test]
fn we_can_correctly_fetch_all_the_referenced_columns() {
    let table_ref = TableRef::new(ResourceId::try_new("sxt", "sxt_tab").unwrap());
    let a = Identifier::try_new("a").unwrap();
    let f = Identifier::try_new("f").unwrap();
    let provable_ast = ProjectionExec::<RistrettoPoint>::new(
        vec![
            aliased_plan(
                DynProofExpr::Column(ColumnExpr::new(ColumnRef::new(
                    table_ref,
                    a,
                    ColumnType::BigInt,
                ))),
                "a",
            ),
            aliased_plan(
                DynProofExpr::Column(ColumnExpr::new(ColumnRef::new(
                    table_ref,
                    f,
                    ColumnType::BigInt,
                ))),
                "f",
            ),
        ],
        TableExpr { table_ref },
    );

    let ref_columns = provable_ast.get_column_references();

    assert_eq!(
        ref_columns,
        IndexSet::from_iter([
            ColumnRef::new(
                table_ref,
                Identifier::try_new("a").unwrap(),
                ColumnType::BigInt
            ),
            ColumnRef::new(
                table_ref,
                Identifier::try_new("f").unwrap(),
                ColumnType::BigInt
            ),
        ])
    );

    let ref_tables = provable_ast.get_table_references();

    assert_eq!(ref_tables, IndexSet::from_iter([table_ref]));
}

#[test]
fn we_can_prove_and_get_the_correct_result_from_a_basic_projection() {
    let data = owned_table([
        bigint("a", [1_i64, 4_i64, 5_i64, 2_i64, 5_i64, 1, 4, 5, 2, 5]),
        bigint("b", [1_i64, 2, 3, 4, 5, 1, 2, 3, 4, 5]),
    ]);
    let t = "sxt.t".parse().unwrap();
    let mut accessor = OwnedTableTestAccessor::<InnerProductProof>::new_empty_with_setup(());
    accessor.add_table(t, data, 0);
    let ast = projection(cols_expr_plan(t, &["b"], &accessor), tab(t));
    let verifiable_res = VerifiableQueryResult::new(&ast, &accessor, &());
    exercise_verification(&verifiable_res, &ast, &accessor, t);
    let res = verifiable_res.verify(&ast, &accessor, &()).unwrap().table;
    let expected = owned_table([bigint("b", [1_i64, 2, 3, 4, 5, 1, 2, 3, 4, 5])]);
    assert_eq!(res, expected);
}

#[test]
fn we_can_prove_and_get_the_correct_result_from_a_nontrivial_projection() {
    let data = owned_table([
        bigint("a", [1_i64, 4_i64, 5_i64, 2_i64, 5_i64]),
        bigint("b", [1_i64, 2, 3, 4, 5]),
    ]);
    let t = "sxt.t".parse().unwrap();
    let mut accessor = OwnedTableTestAccessor::<InnerProductProof>::new_empty_with_setup(());
    accessor.add_table(t, data, 0);
    let ast = projection(
        vec![
            aliased_plan(add(column(t, "b", &accessor), const_bigint(1)), "b"),
            aliased_plan(
                multiply(column(t, "a", &accessor), column(t, "b", &accessor)),
                "prod",
            ),
        ],
        tab(t),
    );
    let verifiable_res = VerifiableQueryResult::new(&ast, &accessor, &());
    exercise_verification(&verifiable_res, &ast, &accessor, t);
    let res = verifiable_res.verify(&ast, &accessor, &()).unwrap().table;
    let expected = owned_table([
        bigint("b", [2_i64, 3, 4, 5, 6]),
        bigint("prod", [1_i64, 8, 15, 8, 25]),
    ]);
    assert_eq!(res, expected);
}

#[test]
fn we_can_get_an_empty_result_from_a_basic_projection_on_an_empty_table_using_result_evaluate() {
    let data = owned_table([
        bigint("a", [0; 0]),
        bigint("b", [0; 0]),
        int128("c", [0; 0]),
        varchar("d", [""; 0]),
        scalar("e", [0; 0]),
    ]);
    let t = "sxt.t".parse().unwrap();
    let mut accessor = OwnedTableTestAccessor::<InnerProductProof>::new_empty_with_setup(());
    accessor.add_table(t, data, 0);
    let expr: DynProofPlan<RistrettoPoint> =
        projection(cols_expr_plan(t, &["b", "c", "d", "e"], &accessor), tab(t));
    let alloc = Bump::new();
    let result_cols = expr.result_evaluate(0, &alloc, &accessor);
    let output_length = result_cols.first().map_or(0, Column::len) as u64;
    let mut builder = FirstRoundBuilder::new();
    expr.first_round_evaluate(&mut builder);
    let fields = &[
        ColumnField::new("b".parse().unwrap(), ColumnType::BigInt),
        ColumnField::new("c".parse().unwrap(), ColumnType::Int128),
        ColumnField::new("d".parse().unwrap(), ColumnType::VarChar),
        ColumnField::new(
            "e".parse().unwrap(),
            ColumnType::Decimal75(Precision::new(75).unwrap(), 0),
        ),
    ];
    let res: OwnedTable<Curve25519Scalar> =
        ProvableQueryResult::new(output_length as u64, &result_cols)
            .to_owned_table(fields)
            .unwrap();
    let expected: OwnedTable<Curve25519Scalar> = owned_table([
        bigint("b", [0; 0]),
        int128("c", [0; 0]),
        varchar("d", [""; 0]),
        decimal75("e", 75, 0, [0; 0]),
    ]);

    assert_eq!(res, expected);
}

#[test]
fn we_can_get_no_columns_from_a_basic_projection_with_no_selected_columns_using_result_evaluate() {
    let data = owned_table([
        bigint("a", [1, 4, 5, 2, 5]),
        bigint("b", [1, 2, 3, 4, 5]),
        int128("c", [1, 2, 3, 4, 5]),
        varchar("d", ["1", "2", "3", "4", "5"]),
        scalar("e", [1, 2, 3, 4, 5]),
    ]);
    let t = "sxt.t".parse().unwrap();
    let mut accessor = OwnedTableTestAccessor::<InnerProductProof>::new_empty_with_setup(());
    accessor.add_table(t, data, 0);
    let expr: DynProofPlan<RistrettoPoint> = projection(cols_expr_plan(t, &[], &accessor), tab(t));
    let alloc = Bump::new();
    let result_cols = expr.result_evaluate(5, &alloc, &accessor);
    let output_length = result_cols.first().map_or(0, Column::len) as u64;
    let mut builder = FirstRoundBuilder::new();
    expr.first_round_evaluate(&mut builder);
    let fields = &[];
    let res: OwnedTable<Curve25519Scalar> =
        ProvableQueryResult::new(output_length as u64, &result_cols)
            .to_owned_table(fields)
            .unwrap();
    let expected = OwnedTable::try_new(IndexMap::default()).unwrap();
    assert_eq!(res, expected);
}

#[test]
fn we_can_get_the_correct_result_from_a_basic_projection_using_result_evaluate() {
    let data = owned_table([
        bigint("a", [1, 4, 5, 2, 5]),
        bigint("b", [1, 2, 3, 4, 5]),
        int128("c", [1, 2, 3, 4, 5]),
        varchar("d", ["1", "2", "3", "4", "5"]),
        scalar("e", [1, 2, 3, 4, 5]),
    ]);
    let t = "sxt.t".parse().unwrap();
    let mut accessor = OwnedTableTestAccessor::<InnerProductProof>::new_empty_with_setup(());
    accessor.add_table(t, data, 0);
    let expr: DynProofPlan<RistrettoPoint> = projection(
        vec![
            aliased_plan(add(column(t, "b", &accessor), const_bigint(1)), "b"),
            aliased_plan(
                multiply(column(t, "b", &accessor), column(t, "c", &accessor)),
                "prod",
            ),
            col_expr_plan(t, "d", &accessor),
            aliased_plan(const_decimal75(1, 0, 3), "e"),
        ],
        tab(t),
    );
    let alloc = Bump::new();
    let result_cols = expr.result_evaluate(5, &alloc, &accessor);
    let output_length = result_cols.first().map_or(0, Column::len) as u64;
    let mut builder = FirstRoundBuilder::new();
    expr.first_round_evaluate(&mut builder);
    let fields = &[
        ColumnField::new("b".parse().unwrap(), ColumnType::BigInt),
        ColumnField::new("prod".parse().unwrap(), ColumnType::Int128),
        ColumnField::new("d".parse().unwrap(), ColumnType::VarChar),
        ColumnField::new(
            "e".parse().unwrap(),
            ColumnType::Decimal75(Precision::new(1).unwrap(), 0),
        ),
    ];
    let res: OwnedTable<Curve25519Scalar> =
        ProvableQueryResult::new(output_length as u64, &result_cols)
            .to_owned_table(fields)
            .unwrap();
    let expected: OwnedTable<Curve25519Scalar> = owned_table([
        bigint("b", [2, 3, 4, 5, 6]),
        int128("prod", [1, 4, 9, 16, 25]),
        varchar("d", ["1", "2", "3", "4", "5"]),
        decimal75("e", 1, 0, [3; 5]),
    ]);
    assert_eq!(res, expected);
}

#[test]
fn we_can_prove_a_projection_on_an_empty_table() {
    let data = owned_table([
        bigint("a", [101; 0]),
        bigint("b", [3; 0]),
        int128("c", [3; 0]),
        varchar("d", ["3"; 0]),
        scalar("e", [3; 0]),
    ]);
    let t = "sxt.t".parse().unwrap();
    let mut accessor = OwnedTableTestAccessor::<InnerProductProof>::new_empty_with_setup(());
    accessor.add_table(t, data, 0);
    let expr = projection(
        vec![
            aliased_plan(add(column(t, "b", &accessor), const_bigint(1)), "b"),
            aliased_plan(
                multiply(column(t, "b", &accessor), column(t, "c", &accessor)),
                "prod",
            ),
            col_expr_plan(t, "d", &accessor),
            aliased_plan(const_decimal75(1, 0, 3), "e"),
        ],
        tab(t),
    );
    let res = VerifiableQueryResult::new(&expr, &accessor, &());
    exercise_verification(&res, &expr, &accessor, t);
    let res = res.verify(&expr, &accessor, &()).unwrap().table;
    let expected = owned_table([
        bigint("b", [3; 0]),
        int128("prod", [3; 0]),
        varchar("d", ["3"; 0]),
        decimal75("e", 1, 0, [3; 0]),
    ]);
    assert_eq!(res, expected);
}

#[test]
fn we_can_prove_a_projection() {
    let data = owned_table([
        bigint("a", [101, 104, 105, 102, 105]),
        bigint("b", [1, 2, 3, 4, 7]),
        int128("c", [1, 3, 3, 4, 5]),
        varchar("d", ["1", "2", "3", "4", "5"]),
        scalar("e", [1, 2, 3, 4, 5]),
    ]);
    let t = "sxt.t".parse().unwrap();
    let mut accessor = OwnedTableTestAccessor::<InnerProductProof>::new_empty_with_setup(());
    accessor.add_table(t, data, 0);
    let expr = projection(
        vec![
            col_expr_plan(t, "b", &accessor),
            col_expr_plan(t, "c", &accessor),
            col_expr_plan(t, "d", &accessor),
            col_expr_plan(t, "e", &accessor),
            aliased_plan(const_int128(105), "const"),
            aliased_plan(
                equal(column(t, "b", &accessor), column(t, "c", &accessor)),
                "bool",
            ),
        ],
        tab(t),
    );
    let res = VerifiableQueryResult::new(&expr, &accessor, &());
    exercise_verification(&res, &expr, &accessor, t);
    let res = res.verify(&expr, &accessor, &()).unwrap().table;
    let expected = owned_table([
        bigint("b", [1, 2, 3, 4, 7]),
        int128("c", [1, 3, 3, 4, 5]),
        varchar("d", ["1", "2", "3", "4", "5"]),
        scalar("e", [1, 2, 3, 4, 5]),
        int128("const", [105; 5]),
        boolean("bool", [true, false, true, true, false]),
    ]);
    assert_eq!(res, expected);
}
