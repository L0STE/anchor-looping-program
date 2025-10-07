use anchor_lang::{prelude::*, solana_program::{instruction::Instruction, program::{invoke, invoke_signed}}};
use anchor_spl::token::{Mint, Token, TokenAccount};
use crate::constant::{JUPITER_PROGRAM_ID, KAMINO_PROGRAM_ID, PROTOCOL_AUTHORITY_BUMP};

const REFRESH_RESERVE_DISCRIMINATOR: [u8; 8] = [2, 218, 138, 235, 79, 201, 25, 102];
const REFRESH_OBLIGATION_DISCRIMINATOR: [u8; 8] = [33, 132, 147, 228, 151, 192, 72, 89];
const WITHDRAW_OBLIGATION_COLLATERAL_AND_REEDEM_RESERVE_COLLATERAL_V2_DISCRIMINATOR: [u8; 8] = [235, 52, 119, 152, 149, 197, 20, 7];
const REPAY_OBLIGATION_LIQUIDITY_V2_DISCRIMINATOR: [u8; 8] = [116, 174, 213, 76, 180, 53, 210, 144];
const SHARED_ACCOUNTS_EXACT_OUT_ROUTE_DISCRIMINATOR: [u8; 8] = [176, 209, 105, 168, 154, 125, 69, 62];
const EXACT_OUT_ROUTE_DISCRIMINATOR: [u8; 8] = [208, 51, 239, 151, 123, 43, 237, 92];

#[derive(Accounts)]
pub struct Repay<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"auth"],
        bump,
    )]
    pub protocol_authority: SystemAccount<'info>,
    pub input_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        associated_token::mint = input_mint,
        associated_token::authority = protocol_authority,
    )]
    pub input_vault: Box<Account<'info, TokenAccount>>,
    pub output_mint: Box<Account<'info, Mint>>,
    #[account(
        mut,
        associated_token::mint = output_mint,
        associated_token::authority = protocol_authority,
    )]
    pub output_vault: Box<Account<'info, TokenAccount>>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: checked by the Kamino program
    pub instruction_sysvar_account: UncheckedAccount<'info>,
    pub token_program: Program<'info, Token>,

    /// Kamino-specific accounts
    #[account(
        mut,
        seeds = [
            b"user_meta", 
            protocol_authority.key().as_ref()           // Owner of the user metadata
        ],
        bump,
        seeds::program = KAMINO_PROGRAM_ID,
    )]
    /// CHECK: checked by the Kamino program
    pub user_metadata: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [
            &[0],                                       // Tag
            &[0],                                       // Id
            protocol_authority.key().as_ref(),          // Obligation owner
            lending_market.key().as_ref(),              // Lending market
            Pubkey::default().as_ref(),                 // Seed1 account
            Pubkey::default().as_ref(),                 // Seed2 account
        ],
        bump,
        seeds::program = KAMINO_PROGRAM_ID,
    )]
    /// CHECK: checked by the Kamino program
    pub obligation: UncheckedAccount<'info>,
    /// CHECK: checked by the Kamino program
    pub lending_market: UncheckedAccount<'info>,
    #[account(
        seeds = [
            b"lma", 
            lending_market.key().as_ref()
        ],
        bump,
        seeds::program = KAMINO_PROGRAM_ID,
    )]
    /// CHECK: checked by the Kamino program
    pub lending_market_authority: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: checked by the Kamino program
    pub reserve_collateral: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: checked by the Kamino program
    pub reserve_liquidity_supply: UncheckedAccount<'info>,
    /// CHECK: checked by the Kamino program
    #[account(mut)]
    pub reserve_collateral_mint: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: checked by the Kamino program
    pub reserve_source_collateral: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: checked by the Kamino program
    pub reserve_borrow: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: checked by the Kamino program
    pub borrow_reserve_destination_liquidity: UncheckedAccount<'info>,
    /// CHECK: checked by the Kamino program
    pub scope_oracle: UncheckedAccount<'info>,
    #[account(
        mut,
        seeds = [
            b"user",
            reserve_farm_state.key().as_ref(),
            obligation.key().as_ref(),
        ],
        bump,
        seeds::program = farms_program.key(),
    )]
    /// CHECK: checked by the Kamino program
    pub obligation_farm_state: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: checked by the Kamino program
    pub reserve_farm_state: UncheckedAccount<'info>,
    #[account(address = KAMINO_PROGRAM_ID)]
    /// CHECK: checked by the Kamino program
    pub kamino_lending_program: AccountInfo<'info>,
    /// CHECK: checked by the Kamino program
    pub farms_program: UncheckedAccount<'info>,

    /// Jupiter-specific accounts
    /// CHECK: checked by the Jupiter program
    pub event_authority: AccountInfo<'info>,
    #[account(address = JUPITER_PROGRAM_ID)]
    /// CHECK: checked by the Jupiter program
    pub jupiter_program: AccountInfo<'info>,

}

impl<'info> Repay<'info> {
    /// # Refresh the reserve collateral
    /// 
    /// This is a step needed to refresh the reserve collateral before interacting with it.
    /// 
    /// We are going to only use scope as oracle (this is Kamino in-house oracle that doesn't require any crank
    /// since they do it for you)
    pub fn refresh_reserve_collateral(&mut self) -> Result<()> {
        let accounts = vec![
            AccountMeta::new(self.reserve_collateral.key(), false),                 // reserve
            AccountMeta::new_readonly(self.lending_market.key(), false),            // lending_market
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),    // [optional] pyth_oracle
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),    // [optional] switchboard_price_oracle
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),    // [optional] switchboard_twap_oracle
            AccountMeta::new_readonly(self.scope_oracle.key(), false),              // [optional] scope_oracle
        ];
        let account_infos = vec![
            self.reserve_collateral.to_account_info(),
            self.lending_market.to_account_info(),
            self.kamino_lending_program.to_account_info(),
            self.scope_oracle.to_account_info(),
        ];

        let refresh_reserve_collateral_ix = Instruction {
            program_id: self.kamino_lending_program.key(),
            accounts,
            data: vec![
                REFRESH_RESERVE_DISCRIMINATOR.as_ref(),
            ].concat(),
        };

        invoke(
            &refresh_reserve_collateral_ix,
            &account_infos,
        )?;

        Ok(())
    }

    /// # Refresh the borrow collateral
    /// 
    /// This is a step needed to refresh the borrow collateral before interacting with it.
    /// 
    /// We are going to only use scope as oracle (this is Kamino in-house oracle that doesn't require any crank
    /// since they do it for you)
    pub fn refresh_reserve_borrow(&mut self) -> Result<()> {
        let accounts = vec![
            AccountMeta::new(self.reserve_borrow.key(), false),                          // reserve
            AccountMeta::new_readonly(self.lending_market.key(), false),            // lending_market
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),    // [optional] pyth_oracle
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),    // [optional] switchboard_price_oracle
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),    // [optional] switchboard_twap_oracle
            AccountMeta::new_readonly(self.scope_oracle.key(), false),              // [optional] scope_oracle
        ];
        let account_infos = vec![
            self.reserve_borrow.to_account_info(),
            self.lending_market.to_account_info(),
            self.kamino_lending_program.to_account_info(),
            self.scope_oracle.to_account_info(),
        ];

        let refresh_reserve_collateral_ix = Instruction {
            program_id: self.kamino_lending_program.key(),
            accounts,
            data: vec![
                REFRESH_RESERVE_DISCRIMINATOR.as_ref(),
            ].concat(),
        };

        invoke(
            &refresh_reserve_collateral_ix,
            &account_infos,
        )?;

        Ok(())
    }

    /// # Refresh the obligation
    /// 
    /// This is a step needed to refresh the obligation before interacting with it.
    /// 
    /// Note: We need to supply as remaining account any cranked reserve account that is used in the obligation for
    /// both collateral and borrows.
    pub fn refresh_obligation(&mut self) -> Result<()> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.lending_market.key(), false),            // lending_market
            AccountMeta::new(self.obligation.key(), false),                         // obligation
        ];
        accounts.push(AccountMeta::new(self.reserve_collateral.key(), false));
        accounts.push(AccountMeta::new(self.reserve_borrow.key(), false));

        let mut account_infos = vec![
            self.lending_market.to_account_info(),
            self.obligation.to_account_info(),
        ];
        account_infos.push(self.reserve_collateral.to_account_info());
        account_infos.push(self.reserve_borrow.to_account_info());

        let refresh_obligation_ix = Instruction {
            program_id: self.kamino_lending_program.key(),
            accounts,
            data: vec![
                REFRESH_OBLIGATION_DISCRIMINATOR.as_ref(),
            ].concat(),
        };

        invoke(
            &refresh_obligation_ix,
            &account_infos,
        )?;

        Ok(())
    }

    pub fn withdraw_collateral(&mut self, amount: u64) -> Result<()> {
        let signer_seeds: [&[&[u8]];1] = [&[
            b"auth".as_ref(),
            &[PROTOCOL_AUTHORITY_BUMP]
        ]];

        let accounts = vec![
            AccountMeta::new(self.protocol_authority.key(), true),                      // owner
            AccountMeta::new(self.obligation.key(), false),                             // obligation
            AccountMeta::new_readonly(self.lending_market.key(), false),                // lending_market
            AccountMeta::new_readonly(self.lending_market_authority.key(), false),      // lending_market_authority
            AccountMeta::new(self.reserve_collateral.key(), false),                     // withdraw_reserve
            AccountMeta::new_readonly(self.input_mint.key(), false),                    // reserve_liquidity_mint
            AccountMeta::new(self.reserve_source_collateral.key(), false),              // reserve_source_collateral
            AccountMeta::new(self.reserve_collateral_mint.key(), false),                // reserve_collateral_mint
            AccountMeta::new(self.reserve_liquidity_supply.key(), false),               // reserve_liquidity_supply
            AccountMeta::new(self.input_vault.key(), false),                            // user_destination_liquidity
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),        // [optional] placeholder_user_destination_collateral
            AccountMeta::new_readonly(self.token_program.key(), false),                 // collateral_token_program
            AccountMeta::new_readonly(self.token_program.key(), false),                 // liquidity_token_program
            AccountMeta::new_readonly(self.instruction_sysvar_account.key(), false),    // instruction_sysvar_account
            AccountMeta::new(self.obligation_farm_state.key(), false),                  // [optional] obligation_farm_user_state
            AccountMeta::new(self.reserve_farm_state.key(), false),                     // [optional] reserve_farm_state
            AccountMeta::new_readonly(self.farms_program.key(), false),                 // farms_program
        ];

        let account_infos = vec![
            self.protocol_authority.to_account_info(),
            self.obligation.to_account_info(),
            self.lending_market.to_account_info(),
            self.lending_market_authority.to_account_info(),
            self.reserve_collateral.to_account_info(),
            self.input_mint.to_account_info(),
            self.reserve_source_collateral.to_account_info(),
            self.reserve_collateral_mint.to_account_info(),
            self.reserve_liquidity_supply.to_account_info(),
            self.input_vault.to_account_info(),
            self.kamino_lending_program.to_account_info(),
            self.token_program.to_account_info(),
            self.instruction_sysvar_account.to_account_info(),
            self.obligation_farm_state.to_account_info(),
            self.reserve_farm_state.to_account_info(),
            self.farms_program.to_account_info(),
        ];

        let borrow_from_collateral_ix = Instruction {
            program_id: self.kamino_lending_program.key(),
            accounts,
            data: vec![
                WITHDRAW_OBLIGATION_COLLATERAL_AND_REEDEM_RESERVE_COLLATERAL_V2_DISCRIMINATOR.as_ref(),
                &amount.to_le_bytes(),
            ].concat(),
        };

        invoke_signed(
            &borrow_from_collateral_ix,
            &account_infos,
            &signer_seeds,
        )?;

        Ok(())
    }

    pub fn swap_for_collateral(&mut self, swap_data: &Vec<u8>, amount: u64, remaining_accounts: &[AccountInfo<'info>]) -> Result<()> {
        // Perform a discriminator, amount and slippage check
        let swap_data_length = swap_data.len();
        let bps_offset = swap_data_length - size_of::<u16>() - size_of::<u8>();
        let amount_offset = bps_offset - size_of::<u64>() - size_of::<u64>();

        require_eq!(amount, u64::from_le_bytes(swap_data[amount_offset..amount_offset + size_of::<u64>()].try_into().unwrap()));
        require_eq!(50, u16::from_le_bytes(swap_data[bps_offset..bps_offset + size_of::<u16>()].try_into().unwrap()));

        let signer_seeds: [&[&[u8]];1] = [&[
            b"auth".as_ref(),
            &[PROTOCOL_AUTHORITY_BUMP]
        ]];

        let (account_infos, accounts) = match swap_data {            
            data if data.starts_with(&EXACT_OUT_ROUTE_DISCRIMINATOR) => {
                let mut account_infos = vec![
                    self.token_program.to_account_info(),
                    self.protocol_authority.to_account_info(),
                    self.input_vault.to_account_info(),
                    self.output_vault.to_account_info(),
                    self.input_mint.to_account_info(),
                    self.output_mint.to_account_info(),
                    self.event_authority.to_account_info(),
                    self.jupiter_program.to_account_info(),
                ];
                account_infos.extend(remaining_accounts.iter().map(|acc| AccountInfo { ..acc.clone() }));

                let mut accounts = vec![
                    AccountMeta::new_readonly(self.token_program.key(), false),         // token program
                    AccountMeta::new_readonly(self.protocol_authority.key(), true),     // user transfer authority
                    AccountMeta::new(self.input_vault.key(), false),                    // user source token account
                    AccountMeta::new(self.output_vault.key(), false),                   // user destination token account
                    AccountMeta::new_readonly(self.jupiter_program.key(), false),       // [optional] destination token account
                    AccountMeta::new_readonly(self.input_mint.key(), false),            // source mint
                    AccountMeta::new_readonly(self.output_mint.key(), false),           // destination mint
                    AccountMeta::new_readonly(self.jupiter_program.key(), false),       // [optional] platform fee account
                    AccountMeta::new_readonly(self.jupiter_program.key(), false),       // [optional] token 2022 program
                    AccountMeta::new_readonly(self.event_authority.key(), false),       // event authority
                    AccountMeta::new_readonly(self.jupiter_program.key(), false),       // jupiter program
                ];
                accounts.extend(remaining_accounts.iter().map(|acc| {
                    AccountMeta {
                        pubkey: *acc.key,
                        is_signer: false,
                        is_writable: acc.is_writable,
                    }
                }));

                (account_infos, accounts)
            }
            data if data.starts_with(&SHARED_ACCOUNTS_EXACT_OUT_ROUTE_DISCRIMINATOR) => {
                // Build the swap instruction accounts
                let mut account_infos = vec![
                    self.token_program.to_account_info(),
                    remaining_accounts[0].to_account_info(),
                    self.protocol_authority.to_account_info(),
                    self.input_vault.to_account_info(),
                    remaining_accounts[1].to_account_info(),
                    remaining_accounts[2].to_account_info(),
                    self.output_vault.to_account_info(),
                    self.input_mint.to_account_info(),
                    self.output_mint.to_account_info(),
                    self.event_authority.to_account_info(),
                    self.jupiter_program.to_account_info(),
                ];
                account_infos.extend(remaining_accounts.iter().map(|acc| AccountInfo { ..acc.clone() }));

                let mut accounts = vec![
                    AccountMeta::new_readonly(self.token_program.key(), false),         // token program
                    AccountMeta::new_readonly(remaining_accounts[0].key(), false),      // program authority
                    AccountMeta::new_readonly(self.protocol_authority.key(), true),     // user transfer authority
                    AccountMeta::new(self.input_vault.key(), false),                    // source token account
                    AccountMeta::new(remaining_accounts[1].key(), false),               // program source token account
                    AccountMeta::new(remaining_accounts[2].key(), false),               // program destination token account
                    AccountMeta::new(self.output_vault.key(), false),                   // destination token account
                    AccountMeta::new_readonly(self.input_mint.key(), false),            // source mint
                    AccountMeta::new_readonly(self.output_mint.key(), false),           // destination mint
                    AccountMeta::new_readonly(self.jupiter_program.key(), false),       // [optional] platform fee account
                    AccountMeta::new_readonly(self.jupiter_program.key(), false),       // [optional] token 2022 program
                    AccountMeta::new_readonly(self.event_authority.key(), false),       // event authority
                    AccountMeta::new_readonly(self.jupiter_program.key(), false),       // jupiter program
                ];
                accounts.extend(remaining_accounts.iter().skip(3).map(|acc| {
                    AccountMeta {
                        pubkey: *acc.key,
                        is_signer: false,
                        is_writable: acc.is_writable,
                    }
                }));
            
                (account_infos, accounts)
            }
            _ => {
                return Err(Error::from(ProgramError::InvalidInstructionData));
            }
        };

        let swap_ix = Instruction {
            program_id: self.jupiter_program.key(),
            accounts,
            data: swap_data.to_vec(),
        };

        invoke_signed(&swap_ix, &account_infos, &signer_seeds)?;

        Ok(())
    }

    pub fn repay_debt(&mut self) -> Result<()> {
        let signer_seeds: [&[&[u8]];1] = [&[
            b"auth".as_ref(),
            &[PROTOCOL_AUTHORITY_BUMP]
        ]];

        let accounts = vec![
            AccountMeta::new(self.protocol_authority.key(), true),                      // owner   
            AccountMeta::new(self.obligation.key(), false),                             // obligation
            AccountMeta::new_readonly(self.lending_market.key(), false),                // lending_market
            AccountMeta::new(self.reserve_borrow.key(), false),                         // repay_reserve
            AccountMeta::new_readonly(self.output_mint.key(), false),                   // reserve_liquidity_mint
            AccountMeta::new(self.borrow_reserve_destination_liquidity.key(), false),   // reserve_destination_liquidity
            AccountMeta::new(self.output_vault.key(), false),                           // user_source_liquidity
            AccountMeta::new_readonly(self.token_program.key(), false),                 // token_program
            AccountMeta::new_readonly(self.instruction_sysvar_account.key(), false),    // instruction_sysvar_account
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),        // [optional] obligation_farm_user_state
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),        // [optional] reserve_farm_state
            AccountMeta::new_readonly(self.lending_market_authority.key(), false),      // lending_market_authority
            AccountMeta::new_readonly(self.farms_program.key(), false),                 // farms_program
        ];

        let account_infos = vec![
            self.protocol_authority.to_account_info(),
            self.obligation.to_account_info(),
            self.lending_market.to_account_info(),
            self.reserve_borrow.to_account_info(),
            self.output_mint.to_account_info(),
            self.borrow_reserve_destination_liquidity.to_account_info(),
            self.output_vault.to_account_info(),
            self.kamino_lending_program.to_account_info(),
            self.token_program.to_account_info(),
            self.instruction_sysvar_account.to_account_info(),
            self.lending_market_authority.to_account_info(),
            self.farms_program.to_account_info(),
        ];

        self.output_vault.reload()?;

        let deposit_ix = Instruction {
            program_id: self.kamino_lending_program.key(),
            accounts,
            data: vec![
                REPAY_OBLIGATION_LIQUIDITY_V2_DISCRIMINATOR.as_ref(),
                &u64::MAX.to_le_bytes(),
            ].concat(),
        };

        invoke_signed(
            &deposit_ix,
            &account_infos,
            &signer_seeds,
        )?;

        Ok(())
    }
}