use anchor_lang::prelude::*;

mod constant;
use crate::constant::{FLAG_HAS_BORROWS, FLAG_HAS_COLLATERAL};
mod instructions;
use instructions::*;

declare_id!("HZ4pzn7pTpkVRpxpszbvBxxQSS11Pu3oYt2PyWW6iFKU");

#[program]
pub mod anchor_looping {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.initialize_user_metadata()?;
        ctx.accounts.initialize_obligation()?;
        ctx.accounts.initialize_obligation_farms_for_reserve()
    }

    pub fn deposit(ctx: Context<Deposit>, has_collateral_or_borrows_flags: u8, amount: u64) -> Result<()> {
        ctx.accounts.refresh_reserve_collateral()?;
        if has_collateral_or_borrows_flags & FLAG_HAS_COLLATERAL != 0 {
            ctx.accounts.refresh_reserve_borrow()?;
        }
        ctx.accounts.refresh_obligation(has_collateral_or_borrows_flags)?;
        ctx.accounts.deposit(amount)
    }

    pub fn looping<'info>(ctx: Context<'_, '_, '_, 'info, Looping<'info>>, has_collateral_or_borrows_flags: u8, swap_data: Vec<u8>, amount: u64) -> Result<()> {
        // Withdraw the collateral to swap
        ctx.accounts.refresh_reserve_collateral()?;
        ctx.accounts.refresh_reserve_borrow()?;
        ctx.accounts.refresh_obligation(has_collateral_or_borrows_flags)?;
        ctx.accounts.borrow_from_collateral(amount)?;

        // Swap the collateral
        ctx.accounts.swap_collateral(swap_data, amount, ctx.remaining_accounts)?;
        
        // Deposit Back the newly swapped collateral
        ctx.accounts.refresh_reserve_collateral()?;
        ctx.accounts.refresh_reserve_borrow()?;
        ctx.accounts.refresh_obligation(FLAG_HAS_BORROWS)?;
        ctx.accounts.deposit()
    }
}
