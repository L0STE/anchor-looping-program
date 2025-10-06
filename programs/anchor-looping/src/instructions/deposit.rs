use anchor_lang::{prelude::*, solana_program::{instruction::Instruction, program::{invoke, invoke_signed}}};
use anchor_spl::token::{Mint, Token, TokenAccount};
use crate::constant::{FLAG_HAS_BORROWS, FLAG_HAS_COLLATERAL, KAMINO_PROGRAM_ID, PROTOCOL_AUTHORITY_BUMP};

const REFRESH_RESERVE_DISCRIMINATOR: [u8; 8] = [2, 218, 138, 235, 79, 201, 25, 102];
const REFRESH_OBLIGATION_DISCRIMINATOR: [u8; 8] = [33, 132, 147, 228, 151, 192, 72, 89];
const DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2_DISCRIMINATOR: [u8; 8] = [216, 224, 191, 27, 204, 151, 102, 175];

#[derive(Accounts)]
pub struct Deposit<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"auth"],
        bump,
    )]
    pub protocol_authority: SystemAccount<'info>,
    pub reserve_liquidity_mint: Account<'info, Mint>,
    #[account(
        mut,
        associated_token::mint = reserve_liquidity_mint,
        associated_token::authority = protocol_authority,
    )]
    pub user_source_liquidity: Account<'info, TokenAccount>,

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
    /// CHECK: checked by the Kamino program
    pub reserve_borrow: Option<UncheckedAccount<'info>>,
    #[account(mut)]
    /// CHECK: checked by the Kamino program
    pub reserve_liquidity_supply: UncheckedAccount<'info>,
    /// CHECK: checked by the Kamino program
    #[account(mut)]
    pub reserve_collateral_mint: UncheckedAccount<'info>,
    #[account(mut)]
    /// CHECK: checked by the Kamino program
    pub reserve_destination_deposit_collateral: UncheckedAccount<'info>,
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
}

impl<'info> Deposit<'info> {
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
    /// 
    /// Note: This is a step needed only if the obligation has any borrows.
    pub fn refresh_reserve_borrow(&mut self) -> Result<()> {
        if let Some(reserve_borrow) = &self.reserve_borrow {
            let accounts = vec![
                AccountMeta::new(reserve_borrow.key(), false),                          // reserve
                AccountMeta::new_readonly(self.lending_market.key(), false),            // lending_market
                AccountMeta::new_readonly(self.kamino_lending_program.key(), false),    // [optional] pyth_oracle
                AccountMeta::new_readonly(self.kamino_lending_program.key(), false),    // [optional] switchboard_price_oracle
                AccountMeta::new_readonly(self.kamino_lending_program.key(), false),    // [optional] switchboard_twap_oracle
                AccountMeta::new_readonly(self.scope_oracle.key(), false),              // [optional] scope_oracle
            ];
            let account_infos = vec![
                reserve_borrow.to_account_info(),
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
        } else {
            Ok(())
        }
    }

    /// # Refresh the obligation
    /// 
    /// This is a step needed to refresh the obligation before interacting with it.
    /// 
    /// Note: We need to supply as remaining account any cranked reserve account that is used in the obligation for
    /// both collateral and borrows.
    pub fn refresh_obligation(&mut self, flags: u8) -> Result<()> {
        let mut accounts = vec![
            AccountMeta::new_readonly(self.lending_market.key(), false),            // lending_market
            AccountMeta::new(self.obligation.key(), false),                         // obligation
        ];
        if flags & FLAG_HAS_COLLATERAL != 0 {
            accounts.push(AccountMeta::new_readonly(self.reserve_collateral.key(), false));
        }
        if flags & FLAG_HAS_BORROWS != 0 {
            if let Some(reserve_borrow) = &self.reserve_borrow {
                accounts.push(AccountMeta::new_readonly(reserve_borrow.key(), false));
            }
        }

        let mut account_infos = vec![
            self.lending_market.to_account_info(),
            self.obligation.to_account_info(),
        ];
        if flags & FLAG_HAS_COLLATERAL != 0 {
            account_infos.push(self.reserve_collateral.to_account_info());
        }
        if flags & FLAG_HAS_BORROWS != 0 {
            if let Some(reserve_borrow) = &self.reserve_borrow {
                account_infos.push(reserve_borrow.to_account_info());
            }
        }

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

    pub fn deposit(&mut self, amount: u64) -> Result<()> {
        let signer_seeds: [&[&[u8]];1] = [&[
            b"auth".as_ref(),
            &[PROTOCOL_AUTHORITY_BUMP]
        ]];

        let accounts = vec![
            AccountMeta::new(self.protocol_authority.key(), true),                      // owner   
            AccountMeta::new(self.obligation.key(), false),                             // obligation
            AccountMeta::new_readonly(self.lending_market.key(), false),                // lending_market
            AccountMeta::new_readonly(self.lending_market_authority.key(), false),      // lending_market_authority
            AccountMeta::new(self.reserve_collateral.key(), false),                     // reserve
            AccountMeta::new_readonly(self.reserve_liquidity_mint.key(), false),        // reserve_liquidity_mint
            AccountMeta::new(self.reserve_liquidity_supply.key(), false),               // reserve_liquidity_supply
            AccountMeta::new(self.reserve_collateral_mint.key(), false),                // reserve_collateral_mint
            AccountMeta::new(self.reserve_destination_deposit_collateral.key(), false), // reserve_destination_deposit_collateral
            AccountMeta::new(self.user_source_liquidity.key(), false),                  // user_source_liquidity
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),        // [optional] placeholder_user_destination_collateral
            AccountMeta::new_readonly(self.token_program.key(), false),                 // collateral_token_program
            AccountMeta::new_readonly(self.token_program.key(), false),                 // liquidity_token_program
            AccountMeta::new_readonly(self.instruction_sysvar_account.key(), false),    // instruction_sysvar_account
            AccountMeta::new_readonly(self.obligation_farm_state.key(), false),         // obligation_farm_user_state
            AccountMeta::new_readonly(self.reserve_farm_state.key(), false),            // reserve_farm_state
            AccountMeta::new_readonly(self.farms_program.key(), false),                 // farms_program
        ];

        let account_infos = vec![
            self.protocol_authority.to_account_info(),
            self.obligation.to_account_info(),
            self.lending_market.to_account_info(),
            self.lending_market_authority.to_account_info(),
            self.reserve_collateral.to_account_info(),
            self.reserve_liquidity_mint.to_account_info(),
            self.reserve_liquidity_supply.to_account_info(),
            self.reserve_collateral_mint.to_account_info(),
            self.reserve_destination_deposit_collateral.to_account_info(),
            self.user_source_liquidity.to_account_info(),
            self.kamino_lending_program.to_account_info(),
            self.token_program.to_account_info(),
            self.token_program.to_account_info(),
            self.instruction_sysvar_account.to_account_info(),
            self.obligation_farm_state.to_account_info(),
            self.reserve_farm_state.to_account_info(),
            self.farms_program.to_account_info(),
        ];

        let deposit_ix = Instruction {
            program_id: self.kamino_lending_program.key(),
            accounts,
            data: vec![
                DEPOSIT_RESERVE_LIQUIDITY_AND_OBLIGATION_COLLATERAL_V2_DISCRIMINATOR.as_ref(),
                &amount.to_le_bytes(),
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