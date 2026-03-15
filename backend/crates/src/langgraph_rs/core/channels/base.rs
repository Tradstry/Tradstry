use serde_json::Value;

use super::ChannelError;

pub trait ChannelClone {
    fn clone_box(&self) -> Box<dyn Channel>;
}

impl<T> ChannelClone for T
where
    T: 'static + Channel + Clone,
{
    fn clone_box(&self) -> Box<dyn Channel> {
        Box::new(self.clone())
    }
}

impl Clone for Box<dyn Channel> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

pub trait Channel: Send + Sync + ChannelClone + core::fmt::Debug {
    fn kind(&self) -> &'static str;

    fn key(&self) -> &str;

    fn set_key(&mut self, key: String);

    fn get(&self) -> Result<Value, ChannelError>;

    fn update(&mut self, values: &[Value]) -> Result<bool, ChannelError>;

    fn consume(&mut self) -> Result<bool, ChannelError> {
        Ok(false)
    }

    fn finish(&mut self) -> Result<bool, ChannelError> {
        Ok(false)
    }

    fn is_available(&self) -> bool {
        self.get().is_ok()
    }

    fn checkpoint(&self) -> Result<Option<Value>, ChannelError> {
        match self.get() {
            Ok(value) => Ok(Some(value)),
            Err(ChannelError::EmptyChannel) => Ok(None),
            Err(err) => Err(err),
        }
    }

    fn from_checkpoint(&self, checkpoint: Option<&Value>)
    -> Result<Box<dyn Channel>, ChannelError>;

    fn copy_boxed(&self) -> Result<Box<dyn Channel>, ChannelError> {
        let checkpoint = self.checkpoint()?;
        self.from_checkpoint(checkpoint.as_ref())
    }
}
