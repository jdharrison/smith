//! Display-only wall times, atomically retained alongside completed history pairs.
use super::*;
use std::collections::BTreeMap;

#[cfg(test)]
#[path = "app_timestamps_tests.rs"]
mod tests;

pub(super) const MAX_TIMESTAMP_MS: u64 = 253_402_300_799_999;

#[derive(Clone, Copy, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct TurnTimes {
    #[serde(deserialize_with = "required_nullable")]
    user_at_ms: Option<u64>,
    #[serde(deserialize_with = "required_nullable")]
    assistant_at_ms: Option<u64>,
}
impl TurnTimes {
    fn sampled(accepted: u64, completed: u64) -> Self {
        Self {
            user_at_ms: valid_time(accepted).then_some(accepted),
            assistant_at_ms: valid_time(completed).then_some(completed),
        }
    }
}
fn valid_time(time: u64) -> bool {
    (1..=MAX_TIMESTAMP_MS).contains(&time)
}

// Missing conversation entries mean unknown times, never inferred historical dates.
pub(super) type Timestamps = BTreeMap<String, Vec<TurnTimes>>;

pub(super) fn validate(times: &Timestamps, conversations: &[v2::Conversation]) -> Result<()> {
    if times.len() > v2::MAX_CONVERSATIONS
        || times.iter().any(|(id, turns)| {
            conversations
                .iter()
                .find(|c| &c.conversation_id == id)
                .is_none_or(|c| turns.len() != c.history.len() / 2)
                || turns.len() > 16
                || turns.iter().any(|t| {
                    t.user_at_ms.is_some_and(|v| !valid_time(v))
                        || t.assistant_at_ms.is_some_and(|v| !valid_time(v))
                })
        })
    {
        return Err("Invalid history timestamps; preserved".into());
    }
    Ok(())
}

fn append_pair(
    times: &mut Timestamps,
    id: &str,
    previous_pairs: usize,
    retained_pairs: usize,
    pair: TurnTimes,
) {
    let turns = times
        .entry(id.to_owned())
        .or_insert_with(|| vec![TurnTimes::default(); previous_pairs]);
    turns.push(pair);
    turns.drain(..turns.len().saturating_sub(retained_pairs));
}

pub(super) fn append(
    times: &mut Timestamps,
    id: &str,
    previous_pairs: usize,
    retained_pairs: usize,
    accepted: u64,
    completed: u64,
) {
    append_pair(
        times,
        id,
        previous_pairs,
        retained_pairs,
        TurnTimes::sampled(accepted, completed),
    );
}

pub(super) fn reserve(times: &mut Timestamps, id: &str, previous_pairs: usize) {
    // Only the preflight clone is changed. Reserve the new map entry, unknown old
    // pairs and longest valid times before admission can spend inference work.
    append_pair(
        times,
        id,
        previous_pairs,
        (previous_pairs + 1).min(16),
        TurnTimes::sampled(MAX_TIMESTAMP_MS, MAX_TIMESTAMP_MS),
    );
}

impl Application {
    pub(super) fn conversation_timestamps(&self, args: Value) -> AppResult<Value> {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct Get {
            conversation_id: String,
        }
        let a: Get = decode(args)?;
        if !valid_id(&a.conversation_id) {
            return Err(invalid());
        }
        let inner = self.inner.lock().map_err(|_| storage())?;
        if inner.fault {
            return Err(storage());
        }
        let c = inner
            .durable
            .conversations
            .iter()
            .find(|c| c.conversation_id == a.conversation_id)
            .ok_or_else(v2::missing)?;
        let turns = inner
            .durable
            .timestamps
            .get(&a.conversation_id)
            .cloned()
            .unwrap_or_else(|| vec![TurnTimes::default(); c.history.len() / 2]);
        Ok(
            json!({"conversation_id":a.conversation_id,"history_revision":c.history_revision,"turns":turns}),
        )
    }
}
