use mcf_ext_opcua::{Attr, Dataset, Dtype, ExtensionTree, Scalar, Value};
use serde_json::json;

fn attr(v: serde_json::Value) -> Result<Attr, serde_json::Error> {
    serde_json::from_value(v)
}

#[test]
fn i64_and_u64_are_decimal_strings() {
    let a = attr(json!({"dtype": "i64", "value": "-42"})).unwrap();
    assert_eq!(a.value, Value::Scalar(Scalar::I64(-42)));
    let b = attr(json!({"dtype": "u64", "value": "18446744073709551615"})).unwrap();
    assert_eq!(b.value, Value::Scalar(Scalar::U64(u64::MAX)));
    assert_eq!(
        serde_json::to_value(&b).unwrap(),
        json!({"dtype": "u64", "value": "18446744073709551615"})
    );
}

#[test]
fn non_finite_floats_use_string_names() {
    let a = attr(json!({"dtype": "f64", "value": ["NaN", "Infinity", "-Infinity", 1.5]})).unwrap();
    let Value::Array(items) = &a.value else {
        panic!("expected array")
    };
    assert!(matches!(items[0], Scalar::F64(v) if v.is_nan()));
    assert_eq!(items[1], Scalar::F64(f64::INFINITY));
    assert_eq!(items[2], Scalar::F64(f64::NEG_INFINITY));
    assert_eq!(
        serde_json::to_value(&a).unwrap(),
        json!({"dtype": "f64", "value": ["NaN", "Infinity", "-Infinity", 1.5]})
    );
}

#[test]
fn empty_array_keeps_its_dtype() {
    let a = attr(json!({"dtype": "i32", "value": []})).unwrap();
    assert_eq!(a.dtype, Dtype::I32);
    assert_eq!(a.value, Value::Array(vec![]));
}

#[test]
fn out_of_range_integer_is_rejected() {
    let e = attr(json!({"dtype": "u32", "value": -1})).unwrap_err();
    assert!(e.to_string().contains("out of range"), "{e}");
    assert!(attr(json!({"dtype": "i8", "value": 128})).is_err());
}

#[test]
fn value_must_match_dtype() {
    assert!(attr(json!({"dtype": "utf8", "value": 1})).is_err());
    assert!(attr(json!({"dtype": "u32", "value": "1"})).is_err());
    assert!(attr(json!({"dtype": "f64", "value": "abc"})).is_err());
    assert!(attr(json!({"dtype": "bogus", "value": 1})).is_err());
}

#[test]
fn unknown_attr_field_is_rejected() {
    assert!(attr(json!({"dtype": "u32", "value": 1, "extra": 1})).is_err());
}

#[test]
fn dataset_shape_must_match_data() {
    let ok: Dataset =
        serde_json::from_value(json!({"dtype": "f32", "shape": [2, 2], "data": [1, 2, 3, 4]}))
            .unwrap();
    assert_eq!(ok.data.len(), 4);
    assert!(serde_json::from_value::<Dataset>(
        json!({"dtype": "f32", "shape": [2, 2], "data": [1, 2, 3]})
    )
    .is_err());
    assert!(serde_json::from_value::<Dataset>(
        json!({"dtype": "utf8", "shape": [1], "data": ["a"]})
    )
    .is_err());
}

#[test]
fn missing_sections_default_to_empty() {
    let t: ExtensionTree = serde_json::from_value(json!({})).unwrap();
    assert_eq!(t, ExtensionTree::default());
}
