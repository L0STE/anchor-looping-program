use anchor_lang::{prelude::*, solana_program::{instruction::Instruction, program::{invoke, invoke_signed}}};
use anchor_spl::token::Token;
use crate::constant::{KAMINO_PROGRAM_ID, PROTOCOL_AUTHORITY_BUMP};

const INIT_USER_METADATA_DISCRIMINATOR: [u8; 8] = [117, 169, 176, 69, 197, 23, 15, 162];
const INIT_OBLIGATION_DISCRIMINATOR: [u8; 8] = [251, 10, 231, 76, 27, 11, 159, 96];
const INIT_OBLIGATION_FARMS_FOR_RESERVE_DISCRIMINATOR: [u8; 8] = [136, 63, 15, 186, 211, 152, 168, 164];

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub payer: Signer<'info>,
    #[account(
        mut,
        seeds = [b"auth"],
        bump = PROTOCOL_AUTHORITY_BUMP,
    )]
    pub protocol_authority: SystemAccount<'info>,

    #[account(address = anchor_lang::solana_program::sysvar::instructions::ID)]
    /// CHECK: checked by the Kamino program
    pub instruction_sysvar_account: UncheckedAccount<'info>,
    pub rent: Sysvar<'info, Rent>,
    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,

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
    pub reserve: UncheckedAccount<'info>,
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

impl<'info> Initialize<'info> {
    /// # Set up the user metadata account
    /// 
    /// This is a step needed for all wallets that use the Kamino program. 
    /// 
    /// In this occasion we don't need to pass in any referrer but if you let user 
    /// interact with Kamino through your program you probably want to pass in one 
    /// of your PDAs as referrer to cash in some rewards.
    pub fn initialize_user_metadata(&mut self) -> Result<()> {
        let signer_seeds: [&[&[u8]];1] = [&[
            b"auth".as_ref(),
            &[PROTOCOL_AUTHORITY_BUMP]
        ]];

        let accounts = vec![
            AccountMeta::new_readonly(self.protocol_authority.key(), true),         // owner
            AccountMeta::new(self.payer.key(), true),                               // fee_payer
            AccountMeta::new(self.user_metadata.key(), false),                      // user metadata account that we are going to initialize
            AccountMeta::new_readonly(self.kamino_lending_program.key(), false),    // [optional] referrer 
            AccountMeta::new_readonly(self.rent.key(), false),                      // rent
            AccountMeta::new_readonly(self.system_program.key(), false),            // system program
        ];

        let account_infos = vec![
            self.protocol_authority.to_account_info(),
            self.payer.to_account_info(),
            self.user_metadata.to_account_info(),
            self.kamino_lending_program.to_account_info(),
            self.rent.to_account_info(),
            self.system_program.to_account_info(),
        ];

        let initialize_user_metadata_ix = Instruction {
            program_id: self.kamino_lending_program.key(),
            accounts,
            data: vec![
                INIT_USER_METADATA_DISCRIMINATOR.as_ref(),
                Pubkey::default().as_ref(),                                         // Lookup Table (used in the frontend)
            ].concat(),
        };
        
        invoke_signed(
            &initialize_user_metadata_ix, 
            &account_infos, 
            &signer_seeds
        )?;

        Ok(())
    }

    /// # Set up the obligation account
    /// 
    /// This is the main account that will be used to borrow and repay assets.
    /// It is also used to store the user's collateral and borrow assets.
    /// 
    /// Some of the inputs are used in their frontend and for this reason we pass in
    /// Pubkey::default().as_ref(), or 0u8 as some of the parameters. 
    pub fn initialize_obligation(&mut self) -> Result<()> {
        let signer_seeds: [&[&[u8]];1] = [&[
            b"auth".as_ref(),
            &[PROTOCOL_AUTHORITY_BUMP]
        ]];

        let accounts = vec![
            AccountMeta::new_readonly(self.protocol_authority.key(), true),         // obbligation_owner
            AccountMeta::new(self.payer.key(), true),                               // fee_payer
            AccountMeta::new(self.obligation.key(), false),                         // obligation account that we are going to initialize
            AccountMeta::new_readonly(self.lending_market.key(), false),            // lending_market
            AccountMeta::new_readonly(self.system_program.key(), false),            // seed1_account (used in the frontend)
            AccountMeta::new_readonly(self.system_program.key(), false),            // seed2_account (used in the frontend)
            AccountMeta::new_readonly(self.user_metadata.key(), false),             // owner_user_metadata
            AccountMeta::new_readonly(self.rent.key(), false),                      // rent
            AccountMeta::new_readonly(self.system_program.key(), false),            // system_program
        ];

        let account_infos = vec![
            self.protocol_authority.to_account_info(),
            self.payer.to_account_info(),
            self.obligation.to_account_info(),
            self.lending_market.to_account_info(),
            self.user_metadata.to_account_info(),
            self.rent.to_account_info(),
            self.system_program.to_account_info(),
        ];

        let initialize_obligation_ix = Instruction {
            program_id: self.kamino_lending_program.key(),
            accounts,
            data: vec![
                INIT_OBLIGATION_DISCRIMINATOR.as_ref(),
                &[0],                                                               // Tag (used in the frontend)
                &[0],                                                               // Id (used in the frontend)
            ].concat(),
        };

        invoke_signed(
            &initialize_obligation_ix, 
            &account_infos, 
            &signer_seeds
        )?;

        Ok(())
    }

    /// # Set up the obligation farms for the reserve
    /// 
    /// This is an account that is needed only if there is a farm on the reserve.
    pub fn initialize_obligation_farms_for_reserve(&mut self) -> Result<()> {
        let accounts = vec![
            AccountMeta::new(self.payer.key(), true),                              // payer
            AccountMeta::new_readonly(self.protocol_authority.key(), false),       // owner
            AccountMeta::new(self.obligation.key(), false),                        // obligation
            AccountMeta::new_readonly(self.lending_market_authority.key(), false), // lending_market_authority
            AccountMeta::new(self.reserve.key(), false),                           // reserve
            AccountMeta::new(self.reserve_farm_state.key(), false),                // reserve_farm_state
            AccountMeta::new(self.obligation_farm_state.key(), false),             // obligation_farm account that we are going to initialize
            AccountMeta::new_readonly(self.lending_market.key(), false),           // lending_market
            AccountMeta::new_readonly(self.farms_program.key(), false),            // farms_program
            AccountMeta::new_readonly(self.rent.key(), false),                     // rent
            AccountMeta::new_readonly(self.system_program.key(), false),           // system_program
        ];

        let account_infos = vec![
            self.payer.to_account_info(),
            self.protocol_authority.to_account_info(),
            self.obligation.to_account_info(),
            self.lending_market_authority.to_account_info(),
            self.reserve.to_account_info(),
            self.reserve_farm_state.to_account_info(),
            self.obligation_farm_state.to_account_info(),
            self.lending_market.to_account_info(),
            self.farms_program.to_account_info(),
            self.rent.to_account_info(),
            self.system_program.to_account_info(),
        ];

        let initialize_obligation_farms_for_reserve_ix = Instruction {
            program_id: self.kamino_lending_program.key(),
            accounts,
            data: vec![
                INIT_OBLIGATION_FARMS_FOR_RESERVE_DISCRIMINATOR.as_ref(),
                &[0],                                                               // Mode (used in the frontend)
            ].concat(),
        };

        invoke(
            &initialize_obligation_farms_for_reserve_ix, 
            &account_infos, 
        )?;

        Ok(())
    }
}