// T104 family: steering — drain_steering/append_steering_notes FIFO wiring. Moved bytes byte-identical (T84 rule)
// from driver.rs's test module; every test here lives in exactly one family
// file.
    use super::*; // the shared harness (driver::tests) + driver's own imports
    #[test]
    fn steering_drains_fifo_into_transcript() {
        let tmp = tempfile::tempdir().unwrap();
        let (tx, rx) = mpsc::channel();
        tx.send("note one".to_string()).unwrap();
        tx.send("note two".to_string()).unwrap();
        drop(tx);

        let notes = drain_steering(&rx);
        assert_eq!(notes, vec!["note one".to_string(), "note two".to_string()]);

        let mut messages = vec![Message::user(vec![ContentBlock::text_block("start")])];
        let mut sink = RecordingSink::default();
        append_steering_notes(tmp.path(), &mut messages, &notes, &mut sink).unwrap();

        assert_eq!(messages.len(), 3);
        for (i, expected) in ["[operator] note one", "[operator] note two"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(messages[1 + i].role, "user");
            assert_eq!(messages[1 + i].content[0].text(), Some(expected));
        }
        // transcript file round-trips exactly what the driver holds
        assert_eq!(transcript::load(tmp.path()).unwrap(), messages[1..]);
        // SteeringQueued emitted in FIFO order
        let queued: Vec<String> = sink
            .0
            .iter()
            .filter_map(|e| match e {
                Event::SteeringQueued(n) => Some(n.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(queued, notes);
    }

    #[test]
    fn steering_drain_ignores_disconnected_channel() {
        let (tx, rx) = mpsc::channel::<String>();
        drop(tx);
        assert!(drain_steering(&rx).is_empty());
    }

