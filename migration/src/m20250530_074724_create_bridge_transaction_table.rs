use sea_orm_migration::{
    prelude::*,
    sea_orm::{EnumIter, Iterable},
};
use sea_query::extension::postgres::Type as PostgresType;

// Enum for TransactionType
#[derive(DeriveIden)]
struct TransactionTypeEnum;

#[derive(DeriveIden, EnumIter)]
pub enum TransactionTypeVariants {
    #[sea_orm(iden = "Deposit")]
    Deposit,
    #[sea_orm(iden = "Withdraw")]
    Withdraw,
    #[sea_orm(iden = "ForcedWithdraw")]
    ForcedWithdraw,
}

// Identifiers for the source_transactions table
#[derive(DeriveIden)]
enum SourceTransactions {
    #[sea_orm(iden = "source_transactions")]
    Table,
    Id,
    ChainId,
    DestinationChainId,
    Nonce,
    TransactionType,
    BlockNumber,
    L1Token,
    L2Token,
    L1Address,
    TwineAddress,
    Amount,
    Message,
    TransactionHash,
    Timestamp,
    CreatedAt,
    UpdatedAt,
}

// Identifiers for the transaction_flows table
#[derive(DeriveIden)]
enum TransactionFlows {
    #[sea_orm(iden = "transaction_flows")]
    Table,
    Id,
    ChainId,
    Nonce,
    HandledAt,
    ExecutedAt,
    HandleTxHash,
    ExecuteTxHash,
    HandleBlockNumber,
    ExecuteBlockNumber,
    HandleStatus,
    TransactionOutput,
    IsHandled,
    IsExecuted,
    IsCompleted,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // 1. Create TransactionType enum
        manager
            .create_type(
                PostgresType::create()
                    .as_enum(TransactionTypeEnum)
                    .values(TransactionTypeVariants::iter())
                    .to_owned(),
            )
            .await?;

        // 2. Create source_transactions table
        manager
            .create_table(
                Table::create()
                    .table(SourceTransactions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SourceTransactions::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::ChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SourceTransactions::DestinationChainId).big_integer())
                    .col(
                        ColumnDef::new(SourceTransactions::Nonce)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::TransactionType)
                            .enumeration(TransactionTypeEnum, TransactionTypeVariants::iter())
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::BlockNumber)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::L1Token)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::L2Token)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::L1Address)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::TwineAddress)
                            .string_len(100)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::Amount)
                            .decimal_len(78, 0)
                            .not_null(),
                    )
                    .col(ColumnDef::new(SourceTransactions::Message).binary())
                    .col(ColumnDef::new(SourceTransactions::TransactionHash).string_len(100))
                    .col(ColumnDef::new(SourceTransactions::Timestamp).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(SourceTransactions::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(SourceTransactions::UpdatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .index(
                        Index::create()
                            .name("unique_source_chain_nonce")
                            .col(SourceTransactions::ChainId)
                            .col(SourceTransactions::Nonce)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;

        // 3. Create transaction_flows table
        manager
            .create_table(
                Table::create()
                    .table(TransactionFlows::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(TransactionFlows::Id)
                            .big_integer()
                            .not_null()
                            .auto_increment()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(TransactionFlows::ChainId)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(TransactionFlows::Nonce)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(TransactionFlows::HandledAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(TransactionFlows::ExecutedAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(TransactionFlows::HandleTxHash).string_len(100))
                    .col(ColumnDef::new(TransactionFlows::ExecuteTxHash).string_len(100))
                    .col(ColumnDef::new(TransactionFlows::HandleBlockNumber).big_integer())
                    .col(ColumnDef::new(TransactionFlows::ExecuteBlockNumber).big_integer())
                    .col(ColumnDef::new(TransactionFlows::HandleStatus).small_integer())
                    .col(ColumnDef::new(TransactionFlows::TransactionOutput).binary())
                    .col(
                        ColumnDef::new(TransactionFlows::IsHandled)
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(TransactionFlows::IsExecuted)
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(TransactionFlows::IsCompleted)
                            .boolean()
                            .default(false),
                    )
                    .col(
                        ColumnDef::new(TransactionFlows::CreatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    .col(
                        ColumnDef::new(TransactionFlows::UpdatedAt)
                            .timestamp_with_time_zone()
                            .default(Expr::cust("CURRENT_TIMESTAMP")),
                    )
                    // .foreign_key(
                    //     ForeignKey::create()
                    //         .name("fk_transaction_flows_to_source_transactions")
                    //         .from(
                    //             TransactionFlows::Table,
                    //             (TransactionFlows::ChainId, TransactionFlows::Nonce),
                    //         )
                    //         .to(
                    //             SourceTransactions::Table,
                    //             (SourceTransactions::ChainId, SourceTransactions::Nonce),
                    //         )
                    //         .on_delete(ForeignKeyAction::Cascade)
                    //         .on_update(ForeignKeyAction::Cascade),
                    // )
                    .index(
                        Index::create()
                            .name("unique_flow_chain_nonce")
                            .col(TransactionFlows::ChainId)
                            .col(TransactionFlows::Nonce)
                            .unique(),
                    )
                    .to_owned(),
            )
            .await?;

        // 5. Create indexes for performance
        // Indexes for source_transactions table
        manager
            .create_index(
                Index::create()
                    .name("idx_source_transactions_chain_nonce")
                    .table(SourceTransactions::Table)
                    .col(SourceTransactions::ChainId)
                    .col(SourceTransactions::Nonce)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_source_transactions_type")
                    .table(SourceTransactions::Table)
                    .col(SourceTransactions::TransactionType)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_source_transactions_timestamp")
                    .table(SourceTransactions::Table)
                    .col(SourceTransactions::Timestamp)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_source_transactions_l1_address")
                    .table(SourceTransactions::Table)
                    .col(SourceTransactions::L1Address)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_source_transactions_block_number")
                    .table(SourceTransactions::Table)
                    .col(SourceTransactions::ChainId)
                    .col(SourceTransactions::BlockNumber)
                    .to_owned(),
            )
            .await?;

        // Indexes for transaction_flows table
        manager
            .create_index(
                Index::create()
                    .name("idx_transaction_flows_completed")
                    .table(TransactionFlows::Table)
                    .col(TransactionFlows::IsCompleted)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_transaction_flows_chain_nonce")
                    .table(TransactionFlows::Table)
                    .col(TransactionFlows::ChainId)
                    .col(TransactionFlows::Nonce)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_transaction_flows_status")
                    .table(TransactionFlows::Table)
                    .col(TransactionFlows::IsHandled)
                    .col(TransactionFlows::IsExecuted)
                    .to_owned(),
            )
            .await?;

        // 6. Create views
        let db = manager.get_connection();

        // Successfully handled deposits view
        db.execute_unprepared(
            "CREATE VIEW successful_deposits AS
                SELECT 
                    st.*,
                    tf.handled_at,
                    tf.handle_tx_hash,
                    tf.handle_block_number,
                    tf.handle_status,
                    tf.transaction_output
                FROM source_transactions st
                JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
                WHERE st.transaction_type = 'Deposit'::transaction_type_enum 
                  AND tf.is_handled = TRUE
                  AND tf.handle_status = 1",
        )
        .await?;

        // Failed deposits pending refund view
        db.execute_unprepared(
            "CREATE VIEW failed_deposits_pending_refund AS
                SELECT 
                    st.*,
                    tf.handled_at,
                    tf.handle_tx_hash,
                    tf.handle_block_number,
                    tf.handle_status,
                    tf.transaction_output
                FROM source_transactions st
                JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
                WHERE st.transaction_type = 'Deposit'::transaction_type_enum 
                  AND tf.is_handled = TRUE
                  AND tf.handle_status = 0
                  AND tf.is_executed = FALSE",
        )
        .await?;

        // Refunded deposits view
        db.execute_unprepared(
            "CREATE VIEW refunded_deposits AS
                SELECT 
                    st.*,
                    tf.handled_at,
                    tf.handle_tx_hash,
                    tf.handle_block_number,
                    tf.handle_status,
                    tf.transaction_output,
                    tf.executed_at AS refunded_at,
                    tf.execute_tx_hash AS refund_tx_hash,
                    tf.execute_block_number AS refund_block_number
                FROM source_transactions st
                JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
                WHERE st.transaction_type = 'Deposit'::transaction_type_enum 
                  AND tf.handle_status = 0
                  AND tf.is_executed = TRUE 
                  AND tf.execute_tx_hash IS NOT NULL",
        )
        .await?;

        // All completed deposits view
        db.execute_unprepared(
            "CREATE VIEW completed_deposits AS
                SELECT 
                    st.*,
                    tf.handled_at,
                    tf.handle_tx_hash,
                    tf.handle_block_number,
                    tf.handle_status,
                    tf.transaction_output,
                    tf.executed_at,
                    tf.execute_tx_hash,
                    tf.execute_block_number,
                    CASE 
                        WHEN tf.handle_status = 1 THEN 'successful'
                        WHEN tf.handle_status = 0 AND tf.is_executed = TRUE THEN 'refunded'
                        ELSE 'unknown'
                    END AS deposit_status
                FROM source_transactions st
                JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
                WHERE st.transaction_type = 'Deposit'::transaction_type_enum 
                  AND tf.is_completed = TRUE",
        )
        .await?;

        // Completed withdrawals view
        db.execute_unprepared(
            "CREATE VIEW completed_withdrawals AS
                SELECT 
                    st.*,
                    tf.handled_at,
                    tf.handle_tx_hash,
                    tf.handle_block_number
                FROM source_transactions st
                JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
                WHERE st.transaction_type = 'Withdraw'::transaction_type_enum 
                  AND tf.is_completed = TRUE",
        )
        .await?;

        // Completed forced withdrawals view
        db.execute_unprepared(
            "CREATE VIEW completed_forced_withdrawals AS
                SELECT 
                    st.*,
                    tf.handled_at,
                    tf.handle_tx_hash,
                    tf.handle_block_number,
                    tf.executed_at,
                    tf.execute_tx_hash,
                    tf.execute_block_number
                FROM source_transactions st
                JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
                WHERE st.transaction_type = 'ForcedWithdraw'::transaction_type_enum 
                  AND tf.is_completed = TRUE",
        )
        .await?;

        // Pending transactions view
        db.execute_unprepared(
            "CREATE VIEW pending_transactions AS
                SELECT 
                    st.*,
                    tf.is_handled,
                    tf.is_executed,
                    tf.handled_at,
                    tf.executed_at
                FROM source_transactions st
                JOIN transaction_flows tf ON st.chain_id = tf.chain_id AND st.nonce = tf.nonce
                WHERE tf.is_completed = FALSE",
        )
        .await?;

        // 7. Create update function
        db.execute_unprepared(
            "CREATE OR REPLACE FUNCTION update_transaction_flow(
                p_chain_id BIGINT,
                p_nonce BIGINT,
                p_stage VARCHAR(10), -- 'execute' or 'settle'
                p_tx_hash VARCHAR(100),
                p_block_number BIGINT
            )
            RETURNS VOID AS $$
            BEGIN
                CASE p_stage
                    WHEN 'execute' THEN
                        UPDATE transaction_flows 
                        SET is_executed = TRUE,
                            executed_at = CURRENT_TIMESTAMP,
                            execute_tx_hash = p_tx_hash,
                            execute_block_number = p_block_number,
                            updated_at = CURRENT_TIMESTAMP
                        WHERE chain_id = p_chain_id AND nonce = p_nonce;
                        
                    WHEN 'settle' THEN
                        UPDATE transaction_flows 
                        SET is_settled = TRUE,
                            settled_at = CURRENT_TIMESTAMP,
                            settle_tx_hash = p_tx_hash,
                            settle_block_number = p_block_number,
                            updated_at = CURRENT_TIMESTAMP
                        WHERE chain_id = p_chain_id AND nonce = p_nonce;
                END CASE;
            END;
            $$ LANGUAGE plpgsql",
        )
        .await?;

        // 8. Create trigger to automatically update is_completed
        db.execute_unprepared(
            "CREATE OR REPLACE FUNCTION update_is_completed()
                RETURNS TRIGGER AS $$
                DECLARE
                    tx_type transaction_type_enum;
                BEGIN
                    SELECT transaction_type INTO tx_type 
                    FROM source_transactions 
                    WHERE chain_id = NEW.chain_id AND nonce = NEW.nonce;
                    
                    NEW.is_completed := CASE 
                        WHEN tx_type = 'Deposit'::transaction_type_enum THEN NEW.is_handled OR NEW.is_executed
                        WHEN tx_type = 'Withdraw'::transaction_type_enum THEN NEW.is_handled
                        WHEN tx_type = 'ForcedWithdraw'::transaction_type_enum THEN NEW.is_handled AND NEW.is_executed
                        ELSE FALSE
                    END;
                    RETURN NEW;
                END;
                $$ LANGUAGE plpgsql",
        )
        .await?;

        db.execute_unprepared(
            "CREATE TRIGGER trigger_update_is_completed
                BEFORE INSERT OR UPDATE ON transaction_flows
                FOR EACH ROW
                EXECUTE FUNCTION update_is_completed()",
        )
        .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Drop in reverse order of creation
        let db = manager.get_connection();

        // 1. Drop trigger and functions
        db.execute_unprepared(
            "DROP TRIGGER IF EXISTS trigger_update_is_completed ON transaction_flows",
        )
        .await?;

        db.execute_unprepared("DROP FUNCTION IF EXISTS update_is_completed()")
            .await?;

        db.execute_unprepared(
            "DROP FUNCTION IF EXISTS update_transaction_flow(BIGINT, BIGINT, VARCHAR, VARCHAR, BIGINT)"
        )
        .await?;

        // 2. Drop views
        db.execute_unprepared("DROP VIEW IF EXISTS pending_transactions")
            .await?;

        db.execute_unprepared("DROP VIEW IF EXISTS completed_forced_withdrawals")
            .await?;

        db.execute_unprepared("DROP VIEW IF EXISTS completed_withdrawals")
            .await?;

        db.execute_unprepared("DROP VIEW IF EXISTS completed_deposits")
            .await?;

        db.execute_unprepared("DROP VIEW IF EXISTS refunded_deposits")
            .await?;

        db.execute_unprepared("DROP VIEW IF EXISTS failed_deposits_pending_refund")
            .await?;

        db.execute_unprepared("DROP VIEW IF EXISTS successful_deposits")
            .await?;

        // 3. Drop indexes (they will be dropped with tables, but being explicit)

        manager
            .drop_index(
                Index::drop()
                    .name("idx_transaction_flows_status")
                    .table(TransactionFlows::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_transaction_flows_chain_nonce")
                    .table(TransactionFlows::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_transaction_flows_completed")
                    .table(TransactionFlows::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_source_transactions_block_number")
                    .table(SourceTransactions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_source_transactions_l1_address")
                    .table(SourceTransactions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_source_transactions_timestamp")
                    .table(SourceTransactions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_source_transactions_type")
                    .table(SourceTransactions::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .drop_index(
                Index::drop()
                    .name("idx_source_transactions_chain_nonce")
                    .table(SourceTransactions::Table)
                    .to_owned(),
            )
            .await?;

        // 4. Drop tables (indexes and foreign keys will be dropped automatically)
        manager
            .drop_table(Table::drop().table(TransactionFlows::Table).to_owned())
            .await?;

        manager
            .drop_table(Table::drop().table(SourceTransactions::Table).to_owned())
            .await?;

        // 5. Drop enum type
        manager
            .drop_type(PostgresType::drop().name(TransactionTypeEnum).to_owned())
            .await?;

        Ok(())
    }
}
