#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::{
        core::channels::{BinaryOperatorAggregate, Channel},
        core::types::Overwrite,
    };

    #[test]
    fn binop_overwrite_and_conflict_parity() {
        let mut channel = BinaryOperatorAggregate::add_numeric("sum");

        channel.update(&[json!(1), json!(2), json!(3)]).unwrap();
        assert_eq!(channel.get().unwrap(), json!(6));

        channel
            .update(&[Overwrite::new(json!(10)).into(), json!(99)])
            .unwrap();
        assert_eq!(channel.get().unwrap(), json!(10));

        let err = channel
            .update(&[json!({"__overwrite__": 1}), json!({"__overwrite__": 2})])
            .unwrap_err();
        assert!(format!("{err}").contains("only one Overwrite"));
    }
}
