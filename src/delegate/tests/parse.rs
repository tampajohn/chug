// T109 family: parse — resume/max_tokens payload parsing (T39).
// Moved bytes byte-identical (T84 rule) from delegate.rs's test
// module; every test here lives in exactly one family file.
// T109 req 4 count-pin anchor (see mod.rs's pin): this family's
// #[test] fn count — a dropped `mod parse;` line fails the pin's
// reference to this const to compile.
pub(super) const TEST_COUNT: usize = 2;
    use super::*; // the shared harness (delegate::tests) + delegate's own imports

    /// T58: the parse — absent → `false`; `false` → `false`; `true` → `true`;
    /// a non-boolean (string "true", number 1) is a tool error naming the
    /// boolean requirement, never a silent fresh start (a caller that asked to
    /// resume and silently got a fresh child would lose the prior run).
    #[test]
    fn delegate_resume_parse_absent_false_true_and_rejects_non_boolean() {
        assert!(!delegate_resume(&json!({})).unwrap());
        assert!(!delegate_resume(&json!({"resume": false})).unwrap());
        assert!(delegate_resume(&json!({"resume": true})).unwrap());
        for bad in [json!("true"), json!(1), json!(0)] {
            let err = delegate_resume(&json!({"resume": bad}))
                .unwrap_err()
                .to_string();
            assert!(err.contains("boolean"), "{bad} → {err}");
        }
    }

    /// T39: the parse — absent → `None`; valid → `Some`; `< 1` (0, negative)
    /// → error naming the constraint; non-integer (string, fractional) →
    /// error naming the integer requirement. Zero never parses into `Some(0)`
    /// (which the child CLI would read as "unlimited").
    #[test]
    fn delegate_max_tokens_parse_absent_valid_and_rejects() {
        assert_eq!(delegate_max_tokens(&json!({})).unwrap(), None);
        assert_eq!(
            delegate_max_tokens(&json!({"max_tokens": 250_000})).unwrap(),
            Some(250_000)
        );
        assert_eq!(delegate_max_tokens(&json!({"max_tokens": 1})).unwrap(), Some(1));
        for bad in [json!(0), json!(-5)] {
            let err = delegate_max_tokens(&json!({"max_tokens": bad}))
                .unwrap_err()
                .to_string();
            assert!(err.contains("at least 1"), "{bad} → {err}");
        }
        for bad in [json!("250000"), json!(250000.5)] {
            let err = delegate_max_tokens(&json!({"max_tokens": bad}))
                .unwrap_err()
                .to_string();
            assert!(err.contains("integer"), "{bad} → {err}");
        }
    }

