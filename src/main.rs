mod constants;
use debtswithfriends::{self, Role};
use teloxide::{prelude::*, utils::command::BotCommands};
#[tokio::main]
async fn main() {
    pretty_env_logger::init();
    log::info!("Iniciando bot para bregar deudas con amigos...");

    let bot = Bot::new(constants::TELOXIDE_TOKEN);

    Command::repl(bot, answer).await;
}

#[derive(BotCommands, Clone)]
#[command(
    rename_rule = "lowercase",
    description = "These commands are supported:"
)]
enum Command {
    #[command(description = "muestra este texto.")]
    Help,
    #[command(description = "muestra este texto.")]
    Start,
    #[command(
        description = "asigna el propietario de los comandos /add o /sub. Debes proporcionar el role ('adder' o 'subtractor')."
    )]
    SetMe(String),
    #[command(description = "agregar una cantidad positiva.")]
    Add(String),
    #[command(description = "agregar una cantidad negativa.")]
    Sub(String),
    #[command(
        description = "calcula el nuevo total. Si se proporciona un parámetro, establecerá el total en ese valor."
    )]
    Total,
    #[command(
        description = "elimina el último valor (debes usar 'confirm' para eliminar el valor)."
    )]
    Pop(String),
    #[command(
        description = "restablece el total a cero (debes usar 'confirm' para restablecerlo)."
    )]
    Reset(String),
    #[command(
        description = "restaura toda la base de datos, incluidos los usuarios y los valores guardados (debes usar 'confirm' para restablecerla)."
    )]
    Restore(String),
    #[command(description = "muestra los valores guardados después del último cálculo total.")]
    History,
    #[command(description = "muestra los usuarios y sus comandos asignados.")]
    Clients,
}

async fn answer(bot: Bot, msg: Message, cmd: Command) -> ResponseResult<()> {
    let plan_code = msg.chat.id.to_string();
    let mut curr_plan: debtswithfriends::Plan = match debtswithfriends::Plan::load(plan_code) {
        Some(plan) => plan,
        None => {
            bot.send_message(msg.chat.id, "No se pudo cargar el plan de deudas")
                .await?;
            return Ok(());
        }
    };

    let teloxide_user = &msg.from.unwrap();

    match &cmd {
        Command::Help | Command::Start => {
            bot.send_message(msg.chat.id, Command::descriptions().to_string())
                .await?
        }
        Command::SetMe(role) => {
            let client_role = match role.to_lowercase().as_str() {
                "adder" => debtswithfriends::Role::Adder,
                "subtractor" => debtswithfriends::Role::Subtractor,
                _ => {
                    bot.send_message(
                        msg.chat.id,
                        String::from(
                            "Por favor escriba un role apropiado ('adder' o 'subtractor')",
                        ),
                    )
                    .await?;
                    return Ok(());
                }
            };

            let client = debtswithfriends::Client::new(
                &teloxide_user.id.to_string(),
                &teloxide_user.full_name(),
                client_role,
            );

            match curr_plan.set_client(&client).unwrap() {
                None => {
                    bot.send_message(
                        msg.chat.id,
                        String::from(
                            "Asignación incorrecta.\nPuede que el usuario ya esté registrado con un role o ambos roles están asignados. Utilice el commando /check para revisar.",
                        ),
                    )
                    .await?;
                }
                Some(_) => {
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "El role '{role}' fué asignado correctamente al usuario: {}",
                            client.name
                        ),
                    )
                    .await?;
                }
            };
            return Ok(());
        }
        Command::Add(value) | Command::Sub(value) => {
            if !check_clients(&bot, msg.chat.id.to_string(), &curr_plan).await? {
                return Ok(());
            }

            if value.is_empty() {
                bot.send_message(
                    msg.chat.id,
                    "Por favor introduzca un valor valido".to_string(),
                )
                .await?;
                return Ok(());
            }

            let val: &str = {
                let split_value = value.split(" ").collect::<Vec<_>>();
                split_value.get(0).unwrap().to_owned()
            };

            let parsed_value = match val.parse::<f64>() {
                Ok(val) => match &cmd {
                    Command::Add(_) => val,
                    Command::Sub(_) => val * -1.0,
                    _ => unreachable!(),
                },
                Err(err) => {
                    bot.send_message(msg.chat.id, err.to_string()).await?;
                    return Ok(());
                }
            };

            curr_plan.save_amount(&parsed_value, &teloxide_user.id.to_string())?;

            bot.send_message(
                msg.chat.id,
                format!(
                    "Guardado correctamente.\nEl monto {} aumentó el balance de {}",
                    parsed_value.abs(),
                    curr_plan
                        .get_client_by_role(match &cmd {
                            Command::Add(_) => Role::Adder,
                            Command::Sub(_) => Role::Subtractor,
                            _ => unreachable!(),
                        })
                        .clone()
                        .unwrap_or_default()
                ),
            )
            .await?;

            return Ok(());
        }
        Command::Pop(param) => {
            if !check_clients(&bot, msg.chat.id.to_string(), &curr_plan).await? {
                return Ok(());
            }
            if !confirm_action(&bot, msg.chat.id.to_string(), &param).await? {
                return Ok(());
            }
            match curr_plan.pop(&teloxide_user.id.to_string())? {
                Some(amount) => {
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "El monto {} se eliminó correctamente",
                            amount.amount_value()
                        ),
                    )
                    .await?;
                }
                None => {
                    bot.send_message(msg.chat.id, "No existe ningún monto para eliminar")
                        .await?;
                }
            }
            return Ok(());
        }
        Command::Reset(param) => {
            if !check_clients(&bot, msg.chat.id.to_string(), &curr_plan).await? {
                return Ok(());
            }
            if !confirm_action(&bot, msg.chat.id.to_string(), &param).await? {
                return Ok(());
            }

            curr_plan.reset(&teloxide_user.id.to_string())?;

            bot.send_message(msg.chat.id, "Los montos del plan han sido removidos")
                .await?;

            return Ok(());
        }
        Command::Restore(param) => {
            if !confirm_action(&bot, msg.chat.id.to_string(), &param).await? {
                return Ok(());
            }

            curr_plan.restore()?;

            bot.send_message(
                msg.chat.id,
                "El plan ha sido restaurado a sus valores por defecto.",
            )
            .await?;

            return Ok(());
        }
        Command::Clients => {
            let clients = curr_plan.show_clients();
            bot.send_message(
                msg.chat.id,
                if clients.is_empty() {
                    "No hay clientes configurados aún".to_string()
                } else {
                    clients
                },
            )
            .await?;
            return Ok(());
        }
        Command::Total => {
            if !check_clients(&bot, msg.chat.id.to_string(), &curr_plan).await? {
                return Ok(());
            }
            let total = curr_plan.total(&teloxide_user.id.to_string())?;
            match total {
                Some(amount) => {
                    bot.send_message(
                        msg.chat.id,
                        format!(
                            "El total actual de la deuda es {:.1}$",
                            amount.amount_value()
                        ),
                    )
                    .await?;
                }
                None => {
                    bot.send_message(
                        msg.chat.id,
                        "No existe registro del total. Se asume que podría ser 0.0$",
                    )
                    .await?;
                }
            }
            return Ok(());
        }
        Command::History => {
            if !check_clients(&bot, msg.chat.id.to_string(), &curr_plan).await? {
                return Ok(());
            }
            let history = curr_plan.history(&teloxide_user.id.to_string());
            bot.send_message(msg.chat.id, history.unwrap_or_default())
                .await?;
            return Ok(());
        }
    };

    Ok(())
}

async fn confirm_action(bot: &Bot, chat_id: String, param: &str) -> ResponseResult<bool> {
    if param.to_lowercase() != "confirm" {
        bot.send_message(
            chat_id,
            "Argumento invalido. Debes enviar 'confirm' para ejecutar esta acción",
        )
        .await?;
        return Ok(false);
    }
    return Ok(true);
}

async fn check_clients(
    bot: &Bot,
    chat_id: String,
    plan: &debtswithfriends::Plan,
) -> ResponseResult<bool> {
    if !plan.check_both_clients_set() {
        bot.send_message(
            chat_id,
            "Acción invalida.\nPrimero se debe usar el comando /setme para asignar el rol de ambos clientes"
                .to_string(),
        )
        .await?;
        return Ok(false);
    }

    return Ok(true);
}
