use anyhow::{Context, Result};
use cli_table::{format::Justify, print_stdout, Cell, Color, Row, RowStruct, Style, Table};
use solo_machine_core::{
    ibc::core::ics24_host::identifier::Identifier,
    model::{AccountOperation, OperationType},
    service::BankService,
    DbPool, Event, Signer,
};
use structopt::StructOpt;
use termcolor::{ColorChoice, ColorSpec, StandardStream};
use tokio::sync::mpsc::UnboundedSender;

use crate::command::print_stream;

use super::add_row;

#[derive(Debug, StructOpt)]
pub enum BankCommand {
    /// Mint some tokens on solo machine
    Mint { amount: u32, denom: Identifier },
    /// Burn some tokens on solo machine
    Burn { amount: u32, denom: Identifier },
    /// Fetch account details
    Account { denom: Identifier },
    /// Check current balance on solo machine
    Balance { denom: Identifier },
    /// Check history of operations on solo machine
    History {
        #[structopt(long, default_value = "10")]
        limit: u32,
        #[structopt(long, default_value)]
        offset: u32,
    },
}

impl BankCommand {
    pub async fn execute(
        self,
        db_pool: DbPool,
        signer: impl Signer,
        sender: UnboundedSender<Event>,
        color_choice: ColorChoice,
    ) -> Result<()> {
        let bank_service = BankService::new_with_notifier(db_pool, sender);

        match self {
            Self::Mint { amount, denom } => bank_service.mint(signer, amount, denom.clone()).await,
            Self::Burn { amount, denom } => bank_service.burn(signer, amount, denom.clone()).await,
            Self::Account { denom } => {
                let account = bank_service.account(signer, &denom).await?;

                match account {
                    None => {
                        let mut stdout = StandardStream::stdout(color_choice);
                        print_stream(
                            &mut stdout,
                            ColorSpec::new().set_bold(true).set_fg(Some(Color::Red)),
                            format!("Account with denom `{}` not found!", denom),
                        )
                    }
                    Some(account) => {
                        let mut table = Vec::new();

                        add_row(&mut table, "Address", account.address);
                        add_row(&mut table, "Denom", account.denom);
                        add_row(&mut table, "Balance", account.balance);
                        add_row(&mut table, "Created at", account.created_at);
                        add_row(&mut table, "Updated at", account.updated_at);

                        print_stdout(table.table().color_choice(color_choice))
                            .context("unable to print table to stdout")
                    }
                }
            }
            Self::Balance { denom } => {
                let balance = bank_service.balance(signer, &denom).await?;

                let table = vec![vec![
                    "Balance".cell().bold(true),
                    format!("{} {}", balance, denom).cell(),
                ]]
                .table()
                .color_choice(color_choice);

                print_stdout(table).context("unable to print table to stdout")
            }
            Self::History { limit, offset } => {
                let history = bank_service.history(signer, limit, offset).await?;

                let table = history
                    .into_iter()
                    .map(into_row)
                    .collect::<Vec<RowStruct>>()
                    .table()
                    .title(vec![
                        "ID".cell().bold(true),
                        "Address".cell().bold(true),
                        "Denom".cell().bold(true),
                        "Amount".cell().bold(true),
                        "Type".cell().bold(true),
                        "Time".cell().bold(true),
                    ])
                    .color_choice(color_choice);

                print_stdout(table).context("unable to print table to stdout")
            }
        }
    }
}

fn into_row(operation: AccountOperation) -> RowStruct {
    let color = get_color_for_operation_type(&operation.operation_type);

    vec![
        operation.id.cell().justify(Justify::Right),
        operation.address.cell(),
        operation.denom.cell(),
        operation.amount.cell().justify(Justify::Right),
        operation
            .operation_type
            .cell()
            .foreground_color(Some(color)),
        operation.created_at.cell(),
    ]
    .row()
}

fn get_color_for_operation_type(operation_type: &OperationType) -> Color {
    match operation_type {
        OperationType::Mint | OperationType::Receive { .. } => Color::Green,
        OperationType::Burn | OperationType::Send { .. } => Color::Red,
    }
}
