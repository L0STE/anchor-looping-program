import { Connection, PublicKey } from "@solana/web3.js"
import { KaminoObligation, KaminoMarket } from "@kamino-finance/klend-sdk";
import { Decimal } from "decimal.js";

// Flags to indicate if obligation has collateral or borrows (matching constant.rs)
export const FLAG_HAS_COLLATERAL = 1 << 0;
export const FLAG_HAS_BORROWS = 1 << 1;

export const K_LEND_PROGRAM_ID = new PublicKey("KLend2g3cP87fffoy8q1mQqGKjrxjC8boSyAYavgmjD");
export const K_FARMS_PROGRAM_ID = new PublicKey("FarmsPZpWu9i7Kky8tPN37rs2TpmMrAZrC7S7vJa91Hr");

export const LENDING_MARKET: PublicKey = new PublicKey("7u3HeHxYDLhnCoErrtycNokbQYbWGzLs6JSDqGAv5PfF");
export const LENDING_MARKET_AUTH: PublicKey = PublicKey.findProgramAddressSync([Buffer.from("lma"), LENDING_MARKET.toBuffer()], K_LEND_PROGRAM_ID)[0];
export const SCOPE_ORACLE_ACCOUNT: PublicKey = new PublicKey("3NJYftD5sjVfxSnUdZ1wVML8f3aC6mp1CXCL6L7TnU8C");


export const CBBTC_RESERVE: PublicKey = new PublicKey("37Jk2zkz23vkAYBT66HM2gaqJuNg2nYLsCreQAVt5MWK");
export const CBBTC_SUPPLY_VAULT: PublicKey = new PublicKey("BcPpdmg4vxXSenvkp12XbVp6XnzwKChnzfNa6cQXLW96");
export const CBBTC_COLLATERAL_MINT: PublicKey = new PublicKey("B3ieCZaTUp8qM9zbPqH2WDhzWpwrvHB2Q2aWB25DW97U");
export const CBBTC_COLLATERAL_VAULT: PublicKey = new PublicKey("hY6yiVepYxxv6dpzayYQ9LnhkX7yrFFpep3oxFRKRgi");
export const CBBTC_COLLATERAL_FARM_ADDRESS: PublicKey = new PublicKey("9CinLHLAcMkzs4Ji8pwS2qwyz1LU46A4Ry7BNLGLubxs");

export const USDC_RESERVE: PublicKey = new PublicKey("D6q6wuQSrifJKZYpR1M8R4YawnLDtDsMmWM1NbBmgJ59");
export const USDC_SUPPLY_VAULT: PublicKey = new PublicKey("Bgq7trRgVMeq33yt235zM2onQ4bRDBsY5EWiTetF4qw6");
export const USDC_COLLATERAL_MINT: PublicKey = new PublicKey("B8V6WVjPxW1UGwVDfxH2d2r8SyT4cqn7dQRK6XneVa7D");
export const USDC_FEE_RECEIVER: PublicKey = new PublicKey("BbDUrk1bVtSixgQsPLBJFZEF7mwGstnD5joA1WzYvYFX");

/* Pool Helpers */
export const obligationAccount = (
    protocolAuthority: PublicKey
): PublicKey => {
    return PublicKey.findProgramAddressSync(
        [
            Buffer.from([0]), 
            Buffer.from([0]), 
            protocolAuthority.toBuffer(),
            LENDING_MARKET.toBuffer(),
            PublicKey.default.toBuffer(),
            PublicKey.default.toBuffer()
        ], K_LEND_PROGRAM_ID
    )[0]
}

export const userMetadataAccount = (
    protocolAuthority: PublicKey
): PublicKey => {
    return PublicKey.findProgramAddressSync(
        [Buffer.from("user_meta"), protocolAuthority.toBuffer()], K_LEND_PROGRAM_ID
    )[0]
}

export const obligationFarmStatePdaAccount = (
    farm: PublicKey,
    obligation: PublicKey
): PublicKey => {
    return PublicKey.findProgramAddressSync(
        [Buffer.from("user"), farm.toBuffer(), obligation.toBuffer()], K_FARMS_PROGRAM_ID
    )[0]
}

export async function hasCollateralOrBorrows(connection: Connection, obligationAddress: PublicKey): Promise<number> {
    const lendingMarket = await KaminoMarket.load(connection, LENDING_MARKET, 400);
    const kaminoObligation = await KaminoObligation.load(lendingMarket, obligationAddress);
    
    if (!kaminoObligation) {
        return 255;
    }
    
    // Get current total deposit value from Kamino's stats
    const currentTotalDeposit = kaminoObligation.refreshedStats.userTotalDeposit;
    
    // Get current total borrow value from Kamino's stats
    const currentTotalBorrow = kaminoObligation.refreshedStats.userTotalBorrowBorrowFactorAdjusted;

    // Check if deposits and borrows are non-zero
    const hasCollateral = currentTotalDeposit.gt(0);
    const hasBorrows = currentTotalBorrow.gt(0);
    
    // Create flags based on whether values are non-zero
    let flags = 0;
    if (hasCollateral) {
        flags |= FLAG_HAS_COLLATERAL;
    }
    if (hasBorrows) {
        flags |= FLAG_HAS_BORROWS;
    }

    return flags;
}

export async function calcuateRepaymentAmount(connection: Connection, obligationAddress: PublicKey) {
    const lendingMarket = await KaminoMarket.load(connection, LENDING_MARKET, 400);
    const usdcReserve = lendingMarket.getReserveByAddress(USDC_RESERVE);
    const kaminoObligation = await KaminoObligation.load(lendingMarket, obligationAddress);

    // Get the USDC borrow position to calculate the exact repayment amount
    const usdcBorrow = kaminoObligation.getBorrowByMint(usdcReserve.getLiquidityMint());
    
    if (!usdcBorrow) {
        throw new Error("No USDC borrow found in obligation");
    }

    // The usdcBorrow.amount already includes accrued interest (in lamports)
    // Convert from lamports to token units by dividing by mint factor
    const currentBorrowAmount = usdcBorrow.amount.div(usdcReserve.getMintFactor());
    
    // Add 0.01% buffer like Kamino does to avoid under-repaying due to interest rate estimation
    const repayAmountWithBuffer = currentBorrowAmount.mul(new Decimal('1.0001'));
    
    // Convert to proper units (multiply by 6 decimals for USDC) and round to integer
    const repayAmountInUnits = repayAmountWithBuffer
        .mul(usdcReserve.getMintFactor())
        .toDecimalPlaces(0, Decimal.ROUND_CEIL);

    return repayAmountInUnits;
}