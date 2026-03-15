use std::sync::Mutex;

use crate::langgraph_rs::core::types::StreamEvent;

pub trait RuntimeStream: Send + Sync {
    fn emit(&self, event: StreamEvent);
}

#[derive(Debug, Default)]
pub struct StreamCollector {
    events: Mutex<Vec<StreamEvent>>,
}

impl StreamCollector {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn events(&self) -> Vec<StreamEvent> {
        self.events
            .lock()
            .map(|events| events.clone())
            .unwrap_or_default()
    }

    pub fn clear(&self) {
        if let Ok(mut events) = self.events.lock() {
            events.clear();
        }
    }
}

impl RuntimeStream for StreamCollector {
    fn emit(&self, event: StreamEvent) {
        if let Ok(mut events) = self.events.lock() {
            events.push(event);
        }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use crate::langgraph_rs::core::types::{StreamEvent, StreamMode};

    use super::{RuntimeStream, StreamCollector};

    #[test]
    fn collects_emitted_events_in_order() {
        let collector = StreamCollector::new();
        collector.emit(StreamEvent::new(StreamMode::Debug, json!({"a": 1})));
        collector.emit(StreamEvent::new(StreamMode::Tasks, json!({"b": 2})));

        let events = collector.events();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].mode, StreamMode::Debug);
        assert_eq!(events[1].mode, StreamMode::Tasks);
    }
}
