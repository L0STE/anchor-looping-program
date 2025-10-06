use anchor_lang::prelude::*;

mod constant;
mod instructions;
use instructions::*;

declare_id!("HZ4pzn7pTpkVRpxpszbvBxxQSS11Pu3oYt2PyWW6iFKU");

#[program]
pub mod anchor_looping {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        msg!("Greetings from: {:?}", ctx.program_id);
        Ok(())
    }
}
