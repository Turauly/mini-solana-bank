# Mini Solana Bank

CLI-программа на Rust (ДЗ 3, курс Solana-разработки).
Моделирует базовые идеи Solana: аккаунты, инструкции, обработчик инструкций,
безопасную арифметику и обработку ошибок.

## Запуск

```
cargo run
cargo test
```

## Команды

| Команда | Описание |
|---|---|
| `create <имя> <lamports>` | создать аккаунт |
| `airdrop <имя> <lamports>` | начислить lamports |
| `transfer <от> <кому> <lamports>` | перевести lamports |
| `balance <имя>` | показать баланс |
| `list` | список аккаунтов |
| `exit` | выход |

## Связь с Solana

| Rust | Solana |
|---|---|
| `struct Account { lamports, owner }` | аккаунт Solana |
| `enum Instruction` | типы инструкций |
| `process_instruction` + `match` | точка входа программы |
| `enum BankError` + `Result` | ProgramError / `#[error_code]` в Anchor |
| `checked_add` / `checked_sub` | защита от overflow |
| проверки до изменения состояния | атомарность транзакций |

## Тесты

6 модульных тестов: `test result: ok. 6 passed; 0 failed`