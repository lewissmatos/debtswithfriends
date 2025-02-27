use std::{
    fmt::Display,
    fs::{self, File, OpenOptions},
    io::{self, Write},
    path::Path,
};

use chrono::{DateTime, FixedOffset, Utc};
use serde::{Deserialize, Serialize};
use serde_json;

const DIR_PATH: &str = "database";
type MyResult<T> = Result<T, io::Error>;
#[derive(Serialize, Deserialize, Debug)]
pub struct Plan {
    code: String,
    clients: Vec<Client>,
    amounts: Vec<Amount>,
    total: Option<Amount>,
}
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub enum Role {
    Adder,
    Subtractor,
}

impl Display for Role {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Role::Adder => write!(formatter, "Adder"),
            Role::Subtractor => write!(formatter, "Subtractor"),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Client {
    pub code: String,
    pub name: String,
    pub role: Role,
}

impl Client {
    pub fn new(code: &str, name: &str, role: Role) -> Self {
        Self {
            code: code.to_string(),
            name: name.to_string(),
            role: role,
        }
    }
}

impl Display for Client {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{} | {}", self.name, self.role)
    }
}

impl Default for Client {
    fn default() -> Self {
        Self {
            code: Default::default(),
            name: Default::default(),
            role: Role::Adder,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Amount {
    value: f64,
    saved_by: Client,
    created_at: DateTime<FixedOffset>,
}

impl Display for Amount {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "💲{}, 🗓️{}, 🥷{}",
            self.value,
            self.created_at.format("%d %m %y"),
            self.saved_by.to_string()
        )
    }
}

impl Default for Amount {
    fn default() -> Self {
        Self {
            value: Default::default(),
            saved_by: Default::default(),
            created_at: Default::default(),
        }
    }
}

impl Amount {
    pub fn from(value: f64, saved_by: Client, created_at: DateTime<FixedOffset>) -> Self {
        Self {
            value,
            saved_by,
            created_at,
        }
    }

    pub fn amount_value(&self) -> f64 {
        self.value
    }
}

impl Plan {
    pub fn new(code: &str) -> Self {
        Self {
            code: code.to_string(),
            clients: vec![],
            amounts: vec![],
            total: None,
        }
    }

    pub fn load(code: String) -> Option<Self> {
        let file_path: String = format!("{DIR_PATH}/{code}.json");

        if !Path::new(DIR_PATH).exists() {
            fs::create_dir_all(DIR_PATH).unwrap();
        }

        let raw_data = {
            match fs::read_to_string(&file_path) {
                Ok(content) => Some(content),
                Err(_) => match File::create(&file_path) {
                    Ok(mut file) => {
                        let serialized_plan: String =
                            serde_json::to_string_pretty(&Self::new(&code)).unwrap();

                        if let Err(_) = file.write_all(serialized_plan.as_bytes()) {
                            None
                        } else {
                            Some(serialized_plan)
                        }
                    }
                    Err(_) => None,
                },
            }
        };

        match raw_data {
            None => None,
            Some(content) => {
                let deserialized_plan: Self = serde_json::from_str(&content).unwrap();
                Some(Self {
                    code,
                    clients: deserialized_plan.clients,
                    amounts: Vec::from(deserialized_plan.amounts),
                    total: deserialized_plan.total,
                })
            }
        }
    }
}

impl Plan {
    fn path(&self) -> String {
        format!("{DIR_PATH}/{}.json", self.code)
    }

    pub fn show_amounts(&self) -> String {
        let list = self
            .amounts
            .iter()
            .map(|amount| amount.to_string())
            .collect::<Vec<_>>();

        list.join("\n")
    }

    pub fn plan_clients(&self) -> String {
        let list = self
            .clients
            .iter()
            .map(|client| client.to_string())
            .collect::<Vec<_>>();

        list.join("\n")
    }
}

// Main methods (commands)
impl Plan {
    fn save_changes_to_db(&mut self) -> MyResult<()> {
        let mut file = OpenOptions::new()
            .write(true)
            .truncate(true)
            .create(true)
            .open(&self.path())?;

        file.write_all(serde_json::to_string_pretty(&self).unwrap().as_bytes())?;
        Ok(())
    }

    pub fn set_client<'a>(&mut self, client: &'a Client) -> MyResult<Option<&'a Client>> {
        if Self::check_client_present(self, &client) {
            return Ok(None);
        }

        if self.clients.len() == 2 {
            return Ok(None);
        } else {
            self.clients.push(client.clone());
        }
        self.save_changes_to_db()?;
        Ok(Some(&client))
    }

    pub fn save_amount<'a>(
        &mut self,
        value: &'a f64,
        client_code: &str,
    ) -> MyResult<Option<&'a f64>> {
        if !Self::check_both_clients_set(self) {
            return Ok(None);
        };

        let client = self.get_client(client_code);

        let created_at = Utc::now().with_timezone(&FixedOffset::west_opt(4 * 3600).unwrap());

        let amount = Amount::from(*value, client.clone(), created_at);

        self.amounts.push(amount);
        self.save_changes_to_db()?;

        Ok(Some(&value))
    }

    pub fn pop(&mut self, client_code: &str) -> MyResult<Option<Amount>> {
        let client = self.get_client(client_code);
        let created_at = Utc::now().with_timezone(&FixedOffset::west_opt(4 * 3600).unwrap());
        let removed_amount = self.amounts.pop();

        let new_total = self.total.clone().unwrap_or_default().amount_value()
            - removed_amount.clone().unwrap_or_default().amount_value();

        self.total = Some(Amount::from(new_total, client, created_at));
        self.save_changes_to_db()?;
        Ok(removed_amount)
    }

    pub fn reset(&mut self, client_code: &str) -> MyResult<&mut Self> {
        let client = self.get_client(client_code);
        let created_at = Utc::now().with_timezone(&FixedOffset::west_opt(4 * 3600).unwrap());
        self.amounts = vec![];
        self.total = Some(Amount::from(0.0, client, created_at));
        self.save_changes_to_db()?;
        Ok(self)
    }

    pub fn restore(&mut self, client_code: &str) -> MyResult<&mut Self> {
        let client = self.get_client(client_code);
        let created_at = Utc::now().with_timezone(&FixedOffset::west_opt(4 * 3600).unwrap());
        self.clients = vec![];
        self.amounts = vec![];
        self.total = Some(Amount::from(0.0, client, created_at));
        self.save_changes_to_db()?;
        Ok(self)
    }

    pub fn total(&mut self, client_code: &str) -> MyResult<Option<Amount>> {
        let client = &self.get_client(client_code);
        let created_at = Utc::now().with_timezone(&FixedOffset::west_opt(4 * 3600).unwrap());

        #[allow(unused)]
        let mut total_amount: f64 = 0.0;
        if self.total.is_none() {
            let filtered_values = &self
                .amounts
                .iter()
                .map(|amount| amount.value)
                .collect::<Vec<_>>();

            total_amount = filtered_values.iter().sum::<f64>();
        } else {
            let filtered_values = &self
                .amounts
                .iter()
                .filter_map(|amount| {
                    if let Some(total) = &self.total {
                        if amount.created_at > total.created_at {
                            Some(amount.value)
                        } else {
                            None
                        }
                    } else {
                        None
                    }
                })
                .collect::<Vec<_>>();

            total_amount = filtered_values.iter().sum::<f64>() + self.total.clone().unwrap().value;
        }
        self.total = Some(Amount::from(total_amount, client.clone(), created_at));

        if total_amount == 0.0 {
            self.reset(client.code.as_ref()).unwrap();
        }

        self.save_changes_to_db()?;

        Ok(self.total.clone())
    }

    pub fn history(&mut self, client_code: &str) -> MyResult<String> {
        let amounts = self
            .amounts
            .iter()
            .map(|amount| amount.to_string())
            .collect::<Vec<_>>();

        self.total(client_code).unwrap().unwrap();

        let total = if self.total.is_none() {
            String::from("💲0.0")
        } else {
            self.total.clone().unwrap().to_string()
        };
        self.save_changes_to_db()?;

        Ok(format!("{}\nTotal:\n{}", amounts.join("\n"), total))
    }
}

//Utils
impl Plan {
    fn check_client_present(self: &Self, client: &Client) -> bool {
        let codes = self
            .clients
            .iter()
            .map(|client| client.code.clone())
            .collect::<Vec<_>>();

        codes.contains(&client.code)
    }

    pub fn check_both_clients_set(self: &Self) -> bool {
        self.clients.get(0).is_some() && self.clients.get(1).is_some()
    }

    pub fn get_client_by_role(self: &Self, role: Role) -> Client {
        self.clients
            .iter()
            .find(|client| client.role == role)
            .unwrap()
            .clone()
    }

    fn get_client(self: &Self, client_code: &str) -> Client {
        self.clients
            .iter()
            .find(|client| client.code == client_code)
            .unwrap()
            .clone()
    }
}
