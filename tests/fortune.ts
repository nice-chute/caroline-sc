import * as anchor from '@project-serum/anchor';
import { Fortune } from '../target/types/fortune';
import { Program, BN, IdlAccounts } from "@project-serum/anchor";
import {
  PublicKey, Keypair, SystemProgram, Transaction, TransactionInstruction, LAMPORTS_PER_SOL,
  SYSVAR_RECENT_BLOCKHASHES_PUBKEY,
  SYSVAR_RENT_PUBKEY
} from "@solana/web3.js";
import { TOKEN_PROGRAM_ID, Token, NATIVE_MINT, ASSOCIATED_TOKEN_PROGRAM_ID } from "@solana/spl-token";
import { assert } from "chai";

describe('fortune', () => {

  // Configure the client to use the local cluster.
  const provider = anchor.Provider.env();
  anchor.setProvider(anchor.Provider.env());
  const program = anchor.workspace.Fortune as Program<Fortune>;

  // Users
  const carolineAuth = Keypair.generate();
  const sellerAuth = Keypair.generate();
  const buyerAuth = Keypair.generate();
  const mintAuth = Keypair.generate();

  // Params
  const marketFee = new anchor.BN(25); // 2.5%
  const initAsk = new anchor.BN(LAMPORTS_PER_SOL) // 1 SOL
  const secondAsk = new anchor.BN(LAMPORTS_PER_SOL * 2) // 2 SOL

  // Accounts
  let market = Keypair.generate();
  let buyerNFTAcc = Keypair.generate();
  let sellerNFTAcc = Keypair.generate();
  let marketVault = null;
  let nftVault = null;
  let listing = null;
  let nftAccount = null;
  let nftMint = null;
  let carolineWsol = null;

  // Bumps
  let marketVaultBump = null;
  let nftVaultBump = null;
  let listingBump = null;

  // Testing vars
  let testSellerBalance = null;


  it('Init state', async () => {
    // Airdrop to carolineAuth
    const carolineAuthAirdrop = await provider.connection.requestAirdrop(carolineAuth.publicKey, 100 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(carolineAuthAirdrop);
    // Airdrop to seller
    const sellerAuthAirdrop = await provider.connection.requestAirdrop(sellerAuth.publicKey, 100 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(sellerAuthAirdrop);
    // Airdrop to buyer
    const buyerAuthAirdrop = await provider.connection.requestAirdrop(buyerAuth.publicKey, 100 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(buyerAuthAirdrop);
    // Airdrop to mint
    const mintAuthAirdrop = await provider.connection.requestAirdrop(mintAuth.publicKey, 100 * LAMPORTS_PER_SOL);
    await provider.connection.confirmTransaction(mintAuthAirdrop);

    // calculate ATA
    carolineWsol = await Token.getAssociatedTokenAddress(
      ASSOCIATED_TOKEN_PROGRAM_ID, // always ASSOCIATED_TOKEN_PROGRAM_ID
      TOKEN_PROGRAM_ID, // always TOKEN_PROGRAM_ID
      NATIVE_MINT, // mint
      carolineAuth.publicKey // owner
    );

    // Creator caroline wsol acc
    let tx = new Transaction().add(
      Token.createAssociatedTokenAccountInstruction(
        ASSOCIATED_TOKEN_PROGRAM_ID, // always ASSOCIATED_TOKEN_PROGRAM_ID
        TOKEN_PROGRAM_ID, // always TOKEN_PROGRAM_ID
        NATIVE_MINT, // mint
        carolineWsol, // ata
        carolineAuth.publicKey, // owner of token account
        carolineAuth.publicKey // fee payer
      )
    );
    await provider.connection.sendTransaction(tx, [carolineAuth]);

    // Nft mint
    nftMint = await Token.createMint(
      provider.connection,
      mintAuth,
      mintAuth.publicKey,
      null,
      0,
      TOKEN_PROGRAM_ID
    );
    // Nft account owned by seller
    nftAccount = await nftMint.createAccount(
      sellerAuth.publicKey,
    );
    // Mint 1 nft to account
    await nftMint.mintTo(
      nftAccount,
      mintAuth.publicKey,
      [mintAuth],
      1
    );

    // Market vault PDA
    [marketVault, marketVaultBump] = await PublicKey.findProgramAddress(
      [
        Buffer.from(anchor.utils.bytes.utf8.encode("vault")),
        market.publicKey.toBuffer(),
        NATIVE_MINT.toBuffer(),
      ],
      program.programId
    );
    // NFT vault PDA
    [nftVault, nftVaultBump] = await PublicKey.findProgramAddress(
      [
        Buffer.from(anchor.utils.bytes.utf8.encode("vault")),
        nftMint.publicKey.toBuffer(),
      ],
      program.programId
    );
    // Listing PDA
    [listing, listingBump] = await PublicKey.findProgramAddress(
      [
        Buffer.from(anchor.utils.bytes.utf8.encode("listing")),
        market.publicKey.toBuffer(),
        nftMint.publicKey.toBuffer(),
        sellerAuth.publicKey.toBuffer()
      ],
      program.programId
    );
  });

  it('Create market', async () => {
    const tx = await program.rpc.createMarket(
      marketFee,
      marketVaultBump,
      {
        accounts: {
          signer: carolineAuth.publicKey,
          market: market.publicKey,
          marketVault: marketVault,
          nativeMint: NATIVE_MINT,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY
        },
        signers: [carolineAuth, market]
      });
    // Market initialized correctly
    let _market = await program.account.market.fetch(market.publicKey);
    assert.ok(_market.authority.equals(carolineAuth.publicKey))
    assert.ok(_market.listingFee.eq(marketFee))
    assert.ok(_market.lamportVault.equals(marketVault))
  });

  it('Create listing 1 SOL', async () => {
    const tx = await program.rpc.createListing(
      initAsk,
      listingBump,
      nftVaultBump,
      {
        accounts: {
          signer: sellerAuth.publicKey,
          listing: listing,
          market: market.publicKey,
          nftVault: nftVault,
          nftAccount: nftAccount,
          nftMint: nftMint.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY
        },
        signers: [sellerAuth]
      });
    // Listing initialized correctly
    let _listing = await program.account.listing.fetch(listing);
    assert.ok(_listing.seller.equals(sellerAuth.publicKey))
    assert.ok(_listing.nftMint.equals(nftMint.publicKey))
    assert.ok(_listing.ask.eq(initAsk))
    assert.ok(_listing.lock == false)
    // Nft vault init correctly
    let _nftVault = await provider.connection.getParsedAccountInfo(nftVault)
    assert.ok(_nftVault.value.data['parsed']['info']['mint'] == nftMint.publicKey.toBase58())
    assert.ok(_nftVault.value.data['parsed']['info']['owner'] == nftVault.toBase58())
    // Nft transferred to vault
    let _nftBalance = await provider.connection.getTokenAccountBalance(nftVault)
    assert.ok(_nftBalance.value.uiAmount == 1)
  });

  it('Ask 2 SOL', async () => {
    const tx = await program.rpc.ask(
      secondAsk,
      listingBump,
      {
        accounts: {
          signer: sellerAuth.publicKey,
          listing: listing,
          market: market.publicKey,
          nftMint: nftMint.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [sellerAuth]
      });
    // Listing correct
    let _listing = await program.account.listing.fetch(listing);
    assert.ok(_listing.seller.equals(sellerAuth.publicKey))
    assert.ok(_listing.nftMint.equals(nftMint.publicKey))
    assert.ok(_listing.ask.eq(secondAsk))
    assert.ok(_listing.lock == false)
  });

  it('Close listing', async () => {
    const tx = await program.rpc.closeListing(
      listingBump,
      nftVaultBump,
      {
        accounts: {
          signer: sellerAuth.publicKey,
          signerNftAcc: sellerNFTAcc.publicKey,
          nftVault: nftVault,
          listing: listing,
          market: market.publicKey,
          nftMint: nftMint.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY
        },
        signers: [sellerAuth, sellerNFTAcc]
      });
    // Nft transferred to seller
    let _nftBalance = await provider.connection.getTokenAccountBalance(sellerNFTAcc.publicKey)
    assert.ok(_nftBalance.value.uiAmount == 1)
  });

  it('Create listing 1 SOL', async () => {
    const tx = await program.rpc.createListing(
      initAsk,
      listingBump,
      nftVaultBump,
      {
        accounts: {
          signer: sellerAuth.publicKey,
          listing: listing,
          market: market.publicKey,
          nftVault: nftVault,
          nftAccount: sellerNFTAcc.publicKey,
          nftMint: nftMint.publicKey,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY
        },
        signers: [sellerAuth]
      });
    // Listing initialized correctly
    let _listing = await program.account.listing.fetch(listing);
    assert.ok(_listing.seller.equals(sellerAuth.publicKey))
    assert.ok(_listing.nftMint.equals(nftMint.publicKey))
    assert.ok(_listing.ask.eq(initAsk))
    assert.ok(_listing.lock == false)
    // Nft vault init correctly
    let _nftVault = await provider.connection.getParsedAccountInfo(nftVault)
    assert.ok(_nftVault.value.data['parsed']['info']['mint'] == nftMint.publicKey.toBase58())
    assert.ok(_nftVault.value.data['parsed']['info']['owner'] == nftVault.toBase58())
    // Nft transferred to vault
    let _nftBalance = await provider.connection.getTokenAccountBalance(nftVault)
    assert.ok(_nftBalance.value.uiAmount == 1)
    // Set seller balance before selling
    let _sellerAccount = await provider.connection.getParsedAccountInfo(sellerAuth.publicKey)
    testSellerBalance = _sellerAccount.value.lamports;
  });

  it('Buy for 1 sol', async () => {
    const tx = await program.rpc.buy(
      listingBump,
      marketVaultBump,
      nftVaultBump,
      {
        accounts: {
          signer: buyerAuth.publicKey,
          signerNftAcc: buyerNFTAcc.publicKey,
          listing: listing,
          seller: sellerAuth.publicKey,
          market: market.publicKey,
          marketVault: marketVault,
          nftVault: nftVault,
          nftMint: nftMint.publicKey,
          nativeMint: NATIVE_MINT,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
          rent: SYSVAR_RENT_PUBKEY,
        },
        signers: [buyerAuth, buyerNFTAcc]
      });
    // Nft transferred to buyer
    let _nftBalance = await provider.connection.getTokenAccountBalance(buyerNFTAcc.publicKey)
    assert.ok(_nftBalance.value.uiAmount == 1)
    // Seller got paid rent + ask
    let _sellerAccount = await provider.connection.getParsedAccountInfo(sellerAuth.publicKey)
    let rent = await provider.connection.getMinimumBalanceForRentExemption(240)
    assert.ok(_sellerAccount.value.lamports == parseInt(testSellerBalance) + initAsk.toNumber() + rent)
    // Market got fees
    let fee = (initAsk.toNumber() * marketFee.toNumber()) / 1000
    let _marketVaultBalance = await provider.connection.getTokenAccountBalance(marketVault)
    assert.ok(_marketVaultBalance.value.uiAmount == fee / LAMPORTS_PER_SOL);
  });

  it('Withdraw fees', async () => {
    // Get amount of balance available
    let _marketVaultBalance = await provider.connection.getTokenAccountBalance(marketVault)
    let fee = new anchor.BN(_marketVaultBalance.value.uiAmount * LAMPORTS_PER_SOL);

    const tx = await program.rpc.withdrawFees(
      fee,
      marketVaultBump,
      {
        accounts: {
          signer: carolineAuth.publicKey,
          targetWsolAcc: carolineWsol,
          market: market.publicKey,
          marketVault: marketVault,
          nativeMint: NATIVE_MINT,
          systemProgram: SystemProgram.programId,
          tokenProgram: TOKEN_PROGRAM_ID,
        },
        signers: [carolineAuth]
      });
    // Fees transferred to target
    let _balance = await provider.connection.getTokenAccountBalance(carolineWsol)
    assert.ok(_balance.value.uiAmount * LAMPORTS_PER_SOL == fee.toNumber());
  });
});
