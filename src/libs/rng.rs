use rand::seq::IndexedRandom;

pub fn uuid() -> String {
    use uuid::Uuid;
    Uuid::new_v4().to_string()
}

pub fn user_agent(user_agents: Option<Vec<String>>) -> String {

    let mut rng = rand::rng();
    if let Some(agents) = user_agents {
        if !agents.is_empty() {
            return agents.choose(&mut rng).unwrap().clone();
        }
    }

    uuid()
}