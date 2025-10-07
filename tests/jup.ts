import { TransactionInstruction, PublicKey, Connection, AddressLookupTableAccount } from "@solana/web3.js";

// Jupiter Addresses
export const jupiterProgramId = new PublicKey("JUP6LkbZbjS1jKKwapdHNy74zcZ3tLUZoi5QNyVTaV4");
export const jupiterEventAuthority = new PublicKey("D8cy77BBepLMngZx6ZukaTff5hCt1HrWyKk3Hnd9oitf");

// Jupiter Helpers
export const deserializeInstruction = (instruction: any): TransactionInstruction => {
    return new TransactionInstruction({
        programId: new PublicKey(instruction.programId),
        keys: instruction.accounts.map((key: any) => ({
            pubkey: new PublicKey(key.pubkey),
            isSigner: key.isSigner,
            isWritable: key.isWritable,
        })),
            data: Buffer.from(instruction.data, "base64"),
    });
};

export const getAddressLookupTableAccounts = async (
    keys: string[],
    connection: Connection
): Promise<AddressLookupTableAccount[]> => {
    const addressLookupTableAccountInfos =
    await connection.getMultipleAccountsInfo(
        keys.map((key) => new PublicKey(key))
    );

    return addressLookupTableAccountInfos.reduce((acc, accountInfo, index) => {
    const addressLookupTableAddress = keys[index];
    if (accountInfo) {
        const addressLookupTableAccount = new AddressLookupTableAccount({
            key: new PublicKey(addressLookupTableAddress),
            state: AddressLookupTableAccount.deserialize(accountInfo.data),
        });
        acc.push(addressLookupTableAccount);
    }

    return acc;
  }, new Array<AddressLookupTableAccount>());
};

// Jupiter Instruction Discriminators
const EXACT_OUT_ROUTE_DISCRIMINATOR = [208, 51, 239, 151, 123, 43, 237, 92];
const ROUTE_DISCRIMINATOR = [229, 23, 203, 151, 122, 227, 173, 42];
const SHARED_ACCOUNTS_EXACT_OUT_ROUTE_DISCRIMINATOR = [176, 209, 105, 168, 154, 125, 69, 62];
const SHARED_ACCOUNTS_ROUTE_DISCRIMINATOR = [193, 32, 155, 51, 65, 214, 156, 129];

// Helper function to compare arrays
const arraysEqual = (a: number[], b: number[]): boolean => {
    return a.length === b.length && a.every((val, index) => val === b[index]);
};

// Function to extract remaining accounts based on discriminator
export const extractRemainingAccountsForSwap = (swapInstruction: TransactionInstruction): { remainingAccounts: any[] } => {
    const instructionData = swapInstruction.data;
    
    // Check discriminator (first 8 bytes)
    const discriminator = Array.from(instructionData.slice(0, 8));
        
    let remainingAccounts: any[] = [];
    
    if (arraysEqual(discriminator, ROUTE_DISCRIMINATOR)) {
        // For Route, the first 9 accounts are base accounts
        remainingAccounts = swapInstruction.keys.slice(9);
    } else if (arraysEqual(discriminator, EXACT_OUT_ROUTE_DISCRIMINATOR)) {
        // For ExactOutRoute, the first 11 accounts are base accounts
        remainingAccounts = swapInstruction.keys.slice(11);
    } else if (arraysEqual(discriminator, SHARED_ACCOUNTS_EXACT_OUT_ROUTE_DISCRIMINATOR) || arraysEqual(discriminator, SHARED_ACCOUNTS_ROUTE_DISCRIMINATOR)) {
        // For SharedAccounts (ExactOutRoute or Route)
        // The smart contract expects:
        // - remaining_accounts[0] = program authority (position 1 in Jupiter response)
        // - remaining_accounts[1] = program source token account (position 4 in Jupiter response)  
        // - remaining_accounts[2] = program destination token account (position 5 in Jupiter response)
        // - remaining_accounts[3+] = all other remaining accounts (position 11+ in Jupiter response)
        const programAuthority = swapInstruction.keys[1];      // position 1
        const programSourceTokenAccount = swapInstruction.keys[4]; // position 4
        const programDestinationTokenAccount = swapInstruction.keys[5]; // position 5
        const otherRemainingAccounts = swapInstruction.keys.slice(13); // after position 12 (index 13+)
        
        remainingAccounts = [
            programAuthority,
            programSourceTokenAccount,
            programDestinationTokenAccount,
            ...otherRemainingAccounts
        ];
    } else {
        throw new Error(`Unknown discriminator: ${discriminator}`);
    }
    
    return {
        remainingAccounts
    };
};

export const swap = async (
    inputMint: PublicKey, 
    outputMint: PublicKey, 
    amount: number,
    slippageBps: number,
    exactOutRoute: boolean,
    onlyDirectRoutes: boolean,
    userPublicKey: PublicKey,
    connection: Connection
): Promise<{
    quoteResponse: any,
    swapInstruction: TransactionInstruction,
    addressLookupTableAccounts: AddressLookupTableAccount[]
}> => {
    const quoteResponse = await (
        await fetch(
            `https://lite-api.jup.ag/swap/v1/quote?inputMint=${inputMint.toString()}&outputMint=${outputMint.toString()}&amount=${amount}&slippageBps=${slippageBps}&onlyDirectRoutes=${onlyDirectRoutes}&swapMode=${exactOutRoute ? "ExactOut" : "ExactIn"}&excludeDexes=ZeroFi,HumidiFi,Tessera`,
        )
      ).json();
  
    const instructions: any = await (
        await fetch('https://lite-api.jup.ag/swap/v1/swap-instructions', {
            method: 'POST',
            headers: {
                'Content-Type': 'application/json',
            },
            body: JSON.stringify({
            quoteResponse,
            userPublicKey: userPublicKey.toString(),
            dynamicSlippage: true,
            })
        })
    ).json();
  
    if (instructions.error) {
        throw new Error("Failed to get swap instructions: " + instructions.error);
    }
  
    const {
        swapInstruction: swapInstructionPayload, // The actual swap instruction.
        addressLookupTableAddresses, // The lookup table addresses that you can use if you are using versioned transaction.
    } = instructions;
  
    return {
      quoteResponse,
      swapInstruction: deserializeInstruction(swapInstructionPayload),
      addressLookupTableAccounts: await getAddressLookupTableAccounts(addressLookupTableAddresses, connection)
    };
  };