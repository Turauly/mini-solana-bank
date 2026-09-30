use std::collections::HashMap;
use std::fmt;
use std::io::{self, Write};

// ============================================================
// Mini Solana Bank — CLI-программа на Rust
// Моделирует базовые идеи Solana: аккаунты, инструкции,
// программа-обработчик, безопасная арифметика и ошибки.
// ============================================================

const PROGRAM_ID: &str = "MiniBank111111111111111111111111";
const LAMPORTS_PER_SOL: u64 = 1_000_000_000;

/// Аккаунт — аналог аккаунта в Solana.
/// У каждого аккаунта есть баланс (lamports) и владелец (owner).
#[derive(Debug, Clone)]
struct Account {
    name: String,
    lamports: u64,
    owner: String,
}

/// Инструкция — команда для программы.
/// В Solana каждая транзакция состоит из инструкций.
#[derive(Debug, PartialEq)]
enum Instruction {
    CreateAccount { name: String, lamports: u64 },
    Airdrop { name: String, amount: u64 },
    Transfer { from: String, to: String, amount: u64 },
    Balance { name: String },
    List,
    Help,
    Exit,
}

/// Ошибки программы — аналог ProgramError / #[error_code] в Anchor.
#[derive(Debug, PartialEq)]
enum BankError {
    AccountAlreadyExists(String),
    AccountNotFound(String),
    InsufficientFunds { needed: u64, available: u64 },
    Overflow,
    InvalidAmount(String),
    ZeroAmount,
    SameAccount,
    UnknownCommand(String),
    WrongArguments(&'static str),
}

impl fmt::Display for BankError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            BankError::AccountAlreadyExists(name) => {
                write!(f, "аккаунт '{}' уже существует", name)
            }
            BankError::AccountNotFound(name) => write!(f, "аккаунт '{}' не найден", name),
            BankError::InsufficientFunds { needed, available } => write!(
                f,
                "недостаточно средств: нужно {} lamports, доступно {}",
                needed, available
            ),
            BankError::Overflow => write!(f, "переполнение (overflow) при вычислении баланса"),
            BankError::InvalidAmount(s) => write!(f, "'{}' не является корректным числом", s),
            BankError::ZeroAmount => write!(f, "сумма должна быть больше нуля"),
            BankError::SameAccount => write!(f, "нельзя переводить самому себе"),
            BankError::UnknownCommand(cmd) => {
                write!(f, "неизвестная команда '{}', введите help", cmd)
            }
            BankError::WrongArguments(usage) => {
                write!(f, "неверные аргументы, используйте: {}", usage)
            }
        }
    }
}

/// «Программа» — хранит состояние (аккаунты) и обрабатывает инструкции.
struct Bank {
    accounts: HashMap<String, Account>,
    tx_count: u64,
}

impl Bank {
    fn new() -> Self {
        Bank {
            accounts: HashMap::new(),
            tx_count: 0,
        }
    }

    /// Точка входа — как process_instruction в Solana-программе.
    fn process_instruction(&mut self, ix: Instruction) -> Result<String, BankError> {
        match ix {
            Instruction::CreateAccount { name, lamports } => self.create_account(name, lamports),
            Instruction::Airdrop { name, amount } => self.airdrop(&name, amount),
            Instruction::Transfer { from, to, amount } => self.transfer(&from, &to, amount),
            Instruction::Balance { name } => self.balance(&name),
            Instruction::List => Ok(self.list()),
            Instruction::Help => Ok(help_text()),
            Instruction::Exit => Ok(String::from("Выход...")),
        }
    }

    fn get_account(&self, name: &str) -> Result<&Account, BankError> {
        self.accounts
            .get(name)
            .ok_or_else(|| BankError::AccountNotFound(name.to_string()))
    }

    fn get_account_mut(&mut self, name: &str) -> Result<&mut Account, BankError> {
        self.accounts
            .get_mut(name)
            .ok_or_else(|| BankError::AccountNotFound(name.to_string()))
    }

    fn create_account(&mut self, name: String, lamports: u64) -> Result<String, BankError> {
        if self.accounts.contains_key(&name) {
            return Err(BankError::AccountAlreadyExists(name));
        }
        let account = Account {
            name: name.clone(),
            lamports,
            owner: PROGRAM_ID.to_string(),
        };
        self.accounts.insert(name.clone(), account);
        self.tx_count += 1;
        Ok(format!(
            "[tx #{}] Аккаунт '{}' создан, баланс: {} lamports",
            self.tx_count, name, lamports
        ))
    }

    fn airdrop(&mut self, name: &str, amount: u64) -> Result<String, BankError> {
        if amount == 0 {
            return Err(BankError::ZeroAmount);
        }
        let account = self.get_account_mut(name)?;
        // checked_add вместо "+" — защита от переполнения, как в Solana-программах
        account.lamports = account
            .lamports
            .checked_add(amount)
            .ok_or(BankError::Overflow)?;
        let new_balance = account.lamports;
        self.tx_count += 1;
        Ok(format!(
            "[tx #{}] Airdrop: +{} lamports для '{}', новый баланс: {}",
            self.tx_count, amount, name, new_balance
        ))
    }

    fn transfer(&mut self, from: &str, to: &str, amount: u64) -> Result<String, BankError> {
        if amount == 0 {
            return Err(BankError::ZeroAmount);
        }
        if from == to {
            return Err(BankError::SameAccount);
        }

        // 1. Сначала все проверки и расчёты, ничего не меняя.
        let from_balance = self.get_account(from)?.lamports;
        let to_balance = self.get_account(to)?.lamports;

        let new_from = from_balance
            .checked_sub(amount)
            .ok_or(BankError::InsufficientFunds {
                needed: amount,
                available: from_balance,
            })?;
        let new_to = to_balance.checked_add(amount).ok_or(BankError::Overflow)?;

        // 2. Только если все проверки прошли, меняем состояние.
        //    Так же в Solana: если инструкция падает с ошибкой,
        //    вся транзакция откатывается (атомарность).
        self.get_account_mut(from)?.lamports = new_from;
        self.get_account_mut(to)?.lamports = new_to;
        self.tx_count += 1;

        Ok(format!(
            "[tx #{}] Transfer: {} -> {}, {} lamports",
            self.tx_count, from, to, amount
        ))
    }

    fn balance(&self, name: &str) -> Result<String, BankError> {
        let acc = self.get_account(name)?;
        Ok(format!(
            "{}: {} lamports ({:.9} SOL), owner: {}",
            acc.name,
            acc.lamports,
            lamports_to_sol(acc.lamports),
            acc.owner
        ))
    }

    fn list(&self) -> String {
        if self.accounts.is_empty() {
            return String::from("Аккаунтов пока нет");
        }
        let mut accounts: Vec<&Account> = self.accounts.values().collect();
        accounts.sort_by(|a, b| a.name.cmp(&b.name));

        let mut out = format!(
            "Аккаунтов: {}, транзакций: {}\n",
            accounts.len(),
            self.tx_count
        );
        for (i, acc) in accounts.iter().enumerate() {
            out.push_str(&format!(
                "  {}. {}: {} lamports\n",
                i + 1,
                acc.name,
                acc.lamports
            ));
        }
        out.trim_end().to_string()
    }
}

fn lamports_to_sol(lamports: u64) -> f64 {
    lamports as f64 / LAMPORTS_PER_SOL as f64
}

fn parse_amount(s: &str) -> Result<u64, BankError> {
    s.parse::<u64>()
        .map_err(|_| BankError::InvalidAmount(s.to_string()))
}

/// Разбирает строку пользователя в инструкцию.
/// В Solana это похоже на десериализацию instruction data.
fn parse_instruction(input: &str) -> Result<Instruction, BankError> {
    let parts: Vec<&str> = input.split_whitespace().collect();

    match parts.as_slice() {
        ["create", name, amount] => Ok(Instruction::CreateAccount {
            name: name.to_string(),
            lamports: parse_amount(amount)?,
        }),
        ["create", ..] => Err(BankError::WrongArguments("create <имя> <lamports>")),

        ["airdrop", name, amount] => Ok(Instruction::Airdrop {
            name: name.to_string(),
            amount: parse_amount(amount)?,
        }),
        ["airdrop", ..] => Err(BankError::WrongArguments("airdrop <имя> <lamports>")),

        ["transfer", from, to, amount] => Ok(Instruction::Transfer {
            from: from.to_string(),
            to: to.to_string(),
            amount: parse_amount(amount)?,
        }),
        ["transfer", ..] => Err(BankError::WrongArguments(
            "transfer <от> <кому> <lamports>",
        )),

        ["balance", name] => Ok(Instruction::Balance {
            name: name.to_string(),
        }),
        ["balance", ..] => Err(BankError::WrongArguments("balance <имя>")),

        ["list"] => Ok(Instruction::List),
        ["help"] => Ok(Instruction::Help),
        ["exit"] | ["quit"] => Ok(Instruction::Exit),

        [cmd, ..] => Err(BankError::UnknownCommand(cmd.to_string())),
        [] => Err(BankError::UnknownCommand(String::new())),
    }
}

fn help_text() -> String {
    String::from(
        "Команды:
  create <имя> <lamports>          создать аккаунт
  airdrop <имя> <lamports>         начислить тестовые lamports
  transfer <от> <кому> <lamports>  перевести lamports
  balance <имя>                    показать баланс
  list                             список всех аккаунтов
  help                             помощь
  exit                             выход",
    )
}

fn main() {
    println!("=== Mini Solana Bank ===");
    println!("{}", help_text());

    let mut bank = Bank::new();
    let stdin = io::stdin();

    loop {
        print!("\n> ");
        io::stdout().flush().expect("не удалось вывести текст");

        let mut input = String::new();
        match stdin.read_line(&mut input) {
            Ok(0) => break, // конец ввода
            Ok(_) => {}
            Err(e) => {
                eprintln!("Ошибка чтения: {}", e);
                break;
            }
        }

        let input = input.trim();
        if input.is_empty() {
            continue;
        }

        match parse_instruction(input) {
            Ok(Instruction::Exit) => {
                println!("До встречи!");
                break;
            }
            Ok(ix) => match bank.process_instruction(ix) {
                Ok(msg) => println!("[OK] {}", msg),
                Err(e) => println!("[ОШИБКА] {}", e),
            },
            Err(e) => println!("[ОШИБКА] {}", e),
        }
    }
}

// ============================================================
// Тесты: запускаются командой `cargo test`
// ============================================================
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_transfer() {
        let mut bank = Bank::new();
        bank.create_account("alice".to_string(), 1000).unwrap();
        bank.create_account("bob".to_string(), 0).unwrap();
        bank.transfer("alice", "bob", 300).unwrap();

        assert_eq!(bank.get_account("alice").unwrap().lamports, 700);
        assert_eq!(bank.get_account("bob").unwrap().lamports, 300);
    }

    #[test]
    fn insufficient_funds_does_not_change_state() {
        let mut bank = Bank::new();
        bank.create_account("alice".to_string(), 100).unwrap();
        bank.create_account("bob".to_string(), 0).unwrap();

        let err = bank.transfer("alice", "bob", 500).unwrap_err();
        assert_eq!(
            err,
            BankError::InsufficientFunds {
                needed: 500,
                available: 100
            }
        );
        // Балансы не изменились — «транзакция откатилась»
        assert_eq!(bank.get_account("alice").unwrap().lamports, 100);
        assert_eq!(bank.get_account("bob").unwrap().lamports, 0);
    }

    #[test]
    fn overflow_is_detected() {
        let mut bank = Bank::new();
        bank.create_account("whale".to_string(), u64::MAX).unwrap();
        assert_eq!(bank.airdrop("whale", 1).unwrap_err(), BankError::Overflow);
    }

    #[test]
    fn duplicate_account_is_rejected() {
        let mut bank = Bank::new();
        bank.create_account("alice".to_string(), 10).unwrap();
        let err = bank.create_account("alice".to_string(), 20).unwrap_err();
        assert_eq!(err, BankError::AccountAlreadyExists("alice".to_string()));
    }

    #[test]
    fn parse_transfer_command() {
        let ix = parse_instruction("transfer alice bob 50").unwrap();
        assert_eq!(
            ix,
            Instruction::Transfer {
                from: "alice".to_string(),
                to: "bob".to_string(),
                amount: 50
            }
        );
    }

    #[test]
    fn parse_invalid_amount() {
        let err = parse_instruction("create alice abc").unwrap_err();
        assert_eq!(err, BankError::InvalidAmount("abc".to_string()));
    }
}