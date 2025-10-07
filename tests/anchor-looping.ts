import * as anchor from "@coral-xyz/anchor";
import { Program } from "@coral-xyz/anchor";
import { Connection, Keypair, LAMPORTS_PER_SOL, PublicKey, SYSVAR_RENT_PUBKEY, SYSVAR_INSTRUCTIONS_PUBKEY, SystemProgram, AddressLookupTableProgram, Transaction, CreateLookupTableParams, ExtendLookupTableParams, ComputeBudgetProgram, TransactionMessage, AddressLookupTableAccount, VersionedTransaction } from "@solana/web3.js";
import { AnchorLooping } from "../target/types/anchor_looping";
import { getAssociatedTokenAddressSync, createAssociatedTokenAccountIdempotentInstruction, TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { CBBTC_COLLATERAL_FARM_ADDRESS, LENDING_MARKET, obligationAccount, obligationFarmStatePdaAccount, userMetadataAccount, LENDING_MARKET_AUTH, CBBTC_RESERVE, K_LEND_PROGRAM_ID, K_FARMS_PROGRAM_ID, USDC_RESERVE, SCOPE_ORACLE_ACCOUNT, CBBTC_SUPPLY_VAULT, CBBTC_COLLATERAL_MINT, CBBTC_COLLATERAL_VAULT, hasCollateralOrBorrows, USDC_FEE_RECEIVER, USDC_SUPPLY_VAULT, calcuateRepaymentAmount,  } from "./kamino";
import { extractRemainingAccountsForSwap, jupiterEventAuthority, jupiterProgramId, swap } from "./jup";
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

  let lookupTable: PublicKey;

  it("Create a lookup table", async () => {
    let result = AddressLookupTableProgram.createLookupTable({
      authority: payer,
      payer: payer,
      recentSlot: await program.provider.connection.getSlot(),
    } as CreateLookupTableParams);

    lookupTable = result[1];;

    let tx = new Transaction();
    tx.instructions.push(result[0])
    await program.provider.sendAndConfirm(tx, [payerKeypair], { skipPreflight: true })
  })

  it("Extend lookup table", async () => {
    let tx = new Transaction();
    tx.instructions.push(AddressLookupTableProgram.extendLookupTable({
      lookupTable,
      authority: payer,
      payer: payer,
      addresses: [
        payer,
        protocolAuthority,
        usdcMint,
        cbBtcMint,
        usdcVault, 
        cbBtcVault, 

        userMetadata,
        obligation,
        reserveFarmState,
        obligationFarmState,

        SCOPE_ORACLE_ACCOUNT,

        LENDING_MARKET,
        LENDING_MARKET_AUTH,
        USDC_RESERVE,
        CBBTC_RESERVE,
        USDC_SUPPLY_VAULT,
        USDC_FEE_RECEIVER,

        CBBTC_SUPPLY_VAULT,
        CBBTC_COLLATERAL_MINT,
        CBBTC_COLLATERAL_VAULT,
        CBBTC_COLLATERAL_FARM_ADDRESS,

        SYSVAR_RENT_PUBKEY,
        TOKEN_PROGRAM_ID,
        SystemProgram.programId,
        SYSVAR_INSTRUCTIONS_PUBKEY,
        K_LEND_PROGRAM_ID,
        K_FARMS_PROGRAM_ID,
      ],
    } as ExtendLookupTableParams))

    await program.provider.sendAndConfirm(tx, [payerKeypair])
    await new Promise(r => setTimeout(r, 2000));
  })

  it("Looping", async () => {
    const amount = 100_000_000;
    const swapResult = await swap(usdcMint, cbBtcMint, amount, 50, false, false, protocolAuthority, program.provider.connection);
    const remainingAccounts = extractRemainingAccountsForSwap(swapResult.swapInstruction).remainingAccounts;
    const flag = await hasCollateralOrBorrows(program.provider.connection, obligation);

    const setComputeUnitLImitIx = ComputeBudgetProgram.setComputeUnitLimit({
      units: 1_200_000,
    }); 

    const createUsdcVaultIx = createAssociatedTokenAccountIdempotentInstruction(payer, usdcVault, protocolAuthority, usdcMint);

    const loopingTx = await program.methods.looping(
      flag,
      swapResult.swapInstruction.data,
      new anchor.BN(amount)
    )
    .accountsStrict({
      payer,
      protocolAuthority,
      inputMint: usdcMint,
      inputVault: usdcVault,
      outputMint: cbBtcMint,
      outputVault: cbBtcVault,
      instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
      tokenProgram: TOKEN_PROGRAM_ID,
      userMetadata,
      obligation,
      lendingMarket: LENDING_MARKET,
      lendingMarketAuthority: LENDING_MARKET_AUTH,
      reserveCollateral: CBBTC_RESERVE,
      reserveLiquiditySupply: CBBTC_SUPPLY_VAULT,
      reserveCollateralMint: CBBTC_COLLATERAL_MINT,
      reserveDestinationDepositCollateral: CBBTC_COLLATERAL_VAULT,
      reserveBorrow: USDC_RESERVE,
      borrowReserveSourceLiquidity: USDC_SUPPLY_VAULT,
      borrowReserveLiquidityFeeReceiver: USDC_FEE_RECEIVER,
      scopeOracle: SCOPE_ORACLE_ACCOUNT,
      obligationFarmState,
      reserveFarmState,
      kaminoLendingProgram: K_LEND_PROGRAM_ID,
      farmsProgram: K_FARMS_PROGRAM_ID,
      eventAuthority: jupiterEventAuthority,
      jupiterProgram: jupiterProgramId
    })
    .remainingAccounts(remainingAccounts)
    .instruction();

    const addressLookupTableAccounts: AddressLookupTableAccount[] = [];
    addressLookupTableAccounts.push((await program.provider.connection.getAddressLookupTable(lookupTable)).value);
    addressLookupTableAccounts.push(...swapResult.addressLookupTableAccounts);

    const messageV0 = new TransactionMessage({
      payerKey: payer,
      recentBlockhash: (await program.provider.connection.getLatestBlockhash()).blockhash,
      instructions: [
        setComputeUnitLImitIx,
        createUsdcVaultIx,
        loopingTx,
      ],
    }).compileToV0Message(addressLookupTableAccounts)

    const tx = new VersionedTransaction(messageV0);
    tx.sign([payerKeypair]);

    await program.provider.connection.sendTransaction(tx, {skipPreflight: true});
  });

  it("Repay", async () => {
    const repayAmount = await calcuateRepaymentAmount(program.provider.connection, obligation);
    const swapResult = await swap(cbBtcMint, usdcMint, repayAmount.toNumber(), 50, true, false, protocolAuthority, program.provider.connection);
    const remainingAccounts = extractRemainingAccountsForSwap(swapResult.swapInstruction).remainingAccounts;

    const setComputeUnitLImitIx = ComputeBudgetProgram.setComputeUnitLimit({
      units: 1_200_000,
    }); 

    const repayTx = await program.methods.repay(
      swapResult.swapInstruction.data,
      new anchor.BN(Number(swapResult.quoteResponse.inAmount)),
      new anchor.BN(repayAmount.toNumber())
    ).accountsStrict({
      payer,
      protocolAuthority,
      inputMint: cbBtcMint,
      inputVault: cbBtcVault,
      outputMint: usdcMint,
      outputVault: usdcVault,
      instructionSysvarAccount: SYSVAR_INSTRUCTIONS_PUBKEY,
      tokenProgram: TOKEN_PROGRAM_ID,
      userMetadata,
      obligation,
      lendingMarket: LENDING_MARKET,
      lendingMarketAuthority: LENDING_MARKET_AUTH,
      reserveCollateral: CBBTC_RESERVE,
      reserveLiquiditySupply: CBBTC_SUPPLY_VAULT,
      reserveCollateralMint: CBBTC_COLLATERAL_MINT,
      reserveSourceCollateral: CBBTC_COLLATERAL_VAULT,
      reserveBorrow: USDC_RESERVE,
      borrowReserveDestinationLiquidity: USDC_SUPPLY_VAULT,
      scopeOracle: SCOPE_ORACLE_ACCOUNT,
      obligationFarmState,
      reserveFarmState,
      kaminoLendingProgram: K_LEND_PROGRAM_ID,
      farmsProgram: K_FARMS_PROGRAM_ID,
      eventAuthority: jupiterEventAuthority,
      jupiterProgram: jupiterProgramId
    })
    .remainingAccounts(remainingAccounts)
    .instruction();

    const addressLookupTableAccounts: AddressLookupTableAccount[] = [];
    addressLookupTableAccounts.push((await program.provider.connection.getAddressLookupTable(lookupTable)).value);
    addressLookupTableAccounts.push(...swapResult.addressLookupTableAccounts);

    const messageV0 = new TransactionMessage({
      payerKey: payer,
      recentBlockhash: (await program.provider.connection.getLatestBlockhash()).blockhash,
      instructions: [
        setComputeUnitLImitIx,
        repayTx,
      ],
    }).compileToV0Message(addressLookupTableAccounts)

    const tx = new VersionedTransaction(messageV0);
    tx.sign([payerKeypair]);

    await program.provider.connection.sendTransaction(tx, {skipPreflight: true});
  });
});
