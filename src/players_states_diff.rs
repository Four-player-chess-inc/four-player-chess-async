use four_player_chess::ident::Ident;
use four_player_chess::state::State;
use std::collections::HashMap;

pub(crate) trait PlayersStatesDiff {
    fn diff(&self, other: &HashMap<Ident, State>) -> Option<HashMap<Ident, State>>;
}

impl PlayersStatesDiff for HashMap<Ident, State> {
    fn diff(&self, other: &HashMap<Ident, State>) -> Option<HashMap<Ident, State>> {
        let x = self
            .clone()
            .into_iter()
            .filter(|(k, v)| other.get(&k).map_or(true, |v2| v2 != v))
            .collect::<HashMap<Ident, State>>();
        if x.is_empty() == false {
            return Some(x);
        }
        None
    }
}
