import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, SYSVAR_RENT_PUBKEY, SYSVAR_INSTRUCTIONS_PUBKEY, SystemProgram } from "@solana/web3.js";
import { AnchorLooping } from "../target/types/anchor_looping";
import { getAssociatedTokenAddressSync, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { CBBTC_COLLATERAL_FARM_ADDRESS, LENDING_MARKET, obligationAccount, obligationFarmStatePdaAccount, userMetadataAccount, LENDING_MARKET_AUTH, CBBTC_RESERVE, K_LEND_PROGRAM_ID, K_FARMS_PROGRAM_ID, USDC_RESERVE, SCOPE_ORACLE_ACCOUNT, CBBTC_SUPPLY_VAULT, CBBTC_COLLATERAL_MINT, CBBTC_COLLATERAL_VAULT, hasCollateralOrBorrows,  } from "./kamino";

// Surfnet Helpers
const surfnetAirdrop = async (connection: Connection, address: string, lamports: number) => {
  const call = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "surfnet_setAccount",
    "params": [address, { "lamports": lamports }]
  };
  
  await fetch(connection.rpcEndpoint, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(call)
  });
};

const surfnetTokenAirdrop = async (connection: Connection, owner: string, mint: string, amount: number) => {
  const call = {
    "jsonrpc": "2.0",
    "id": 1,
    "method": "surfnet_setTokenAccount",
    "params": [owner, mint, {"amount": amount, "state": "initialized"}]
  };
  
  await fetch(connection.rpcEndpoint, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
    },
    body: JSON.stringify(call)
  });
};

describe("anchor-looping", () => {
  // Configure the client to use the local cluster.
  anchor.setProvider(anchor.AnchorProvider.env());

  const program = anchor.workspace.anchorLooping as Program<AnchorLooping>;

  // Payer
  const payerKeypair = new Keypair();
  const payer = payerKeypair.publicKey;

  // Mint Addresses
  const usdcMint = new PublicKey("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v");
  const cbBtcMint = new PublicKey("cbbtcf3aa214zXHbiAZQwf4122FBYbraNdFqgw4iMij");

  // Protocol Accounts
  const protocolAuthority = PublicKey.findProgramAddressSync([Buffer.from("auth")], program.programId)[0];
  const usdcVault = getAssociatedTokenAddressSync(usdcMint, protocolAuthority, true);
  const cbBtcVault = getAssociatedTokenAddressSync(cbBtcMint, protocolAuthority, true);

  it("Setup", async () => {
    // Airdrop to payer
    await surfnetAirdrop(program.provider.connection, payer.toString(), 1_000 * LAMPORTS_PER_SOL);
  });

  let userMetadata = userMetadataAccount(protocolAuthority);
  let obligation = obligationAccount(protocolAuthority);
  let reserveFarmState = CBBTC_COLLATERAL_FARM_ADDRESS
  let obligationFarmState = obligationFarmStatePdaAccount(reserveFarmState, obligation);

  it("Initialize Kamino Accounts", async () => {
    await program.methods.initialize()
    .accountsStrict({
      payer,
      protocolAuthority,
      instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
      rent: SYSVAR_RENT_PUBKEY,
      tokenProgram: TOKEN_PROGRAM_ID,
      systemProgram: SystemProgram.programId,
      userMetadata,
      obligation,
      lendingMarket: LENDING_MARKET,
      lendingMarketAuthority: LENDING_MARKET_AUTH,
      reserve: CBBTC_RESERVE,
      obligationFarmState,
      reserveFarmState,
      kaminoLendingProgram: K_LEND_PROGRAM_ID,
      farmsProgram: K_FARMS_PROGRAM_ID
    })
    .signers([payerKeypair])
    .rpc({ skipPreflight: true });
  });

  it("Deposit", async () => {
    // Airdrop cbBTC to protocol authority input vault
    await surfnetTokenAirdrop(program.provider.connection, protocolAuthority.toString(), cbBtcMint.toString(), 100_000_000);

    // Deposit cbBTC to obligation
    let flag = await hasCollateralOrBorrows(program.provider.connection, obligation);
    await program.methods.deposit(
      flag, 
      new anchor.BN(100_000_000)
    ).accountsStrict({
      payer,
      protocolAuthority,
      reserveLiquidityMint: cbBtcMint,
      userSourceLiquidity: cbBtcVault,
      instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
      tokenProgram: TOKEN_PROGRAM_ID,
      userMetadata,
      obligation,
      lendingMarket: LENDING_MARKET,
      lendingMarketAuthority: LENDING_MARKET_AUTH,
      reserveCollateral: CBBTC_RESERVE,
      reserveBorrow: USDC_RESERVE,
      reserveLiquiditySupply: CBBTC_SUPPLY_VAULT,
      reserveCollateralMint: CBBTC_COLLATERAL_MINT,
      reserveDestinationDepositCollateral: CBBTC_COLLATERAL_VAULT,
      scopeOracle: SCOPE_ORACLE_ACCOUNT,
      obligationFarmState,
      reserveFarmState,
      kaminoLendingProgram: K_LEND_PROGRAM_ID,
      farmsProgram: K_FARMS_PROGRAM_ID
    })
    .signers([payerKeypair])
    .rpc({ skipPreflight: true });
  });
});
