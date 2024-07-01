use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let conversations = Table::create()
            .table(Conversations::Table)
            .if_not_exists()
            .col(ColumnDef::new(Conversations::Id).integer().not_null().auto_increment().primary_key())
            .col(ColumnDef::new(Conversations::Uuid).binary_len(16).unique_key().not_null())
            .col(ColumnDef::new(Conversations::LastUpdated).big_integer().not_null())
            .col(ColumnDef::new(Conversations::FirstExchange).integer().unique_key().not_null())
            .foreign_key(ForeignKey::create()
                .from(Conversations::Table, Conversations::FirstExchange)
                .to(Exchanges::Table, Exchanges::Id))
            .to_owned();
        manager.create_table(conversations).await?;

        // let exchanges = Table::create()
        //     .table(Exchanges::Table)
        //     .if_not_exists()
        //     .col(ColumnDef::new(Exchanges::Id).integer().not_null().auto_increment().primary_key())
        //     .col(ColumnDef::new(Exchanges::Key).integer().not_null())
        //     .col(ColumnDef::new(Exchanges::UserMessage).string().not_null())
        //     .col(ColumnDef::new(Exchanges::AssistantMessage).string().not_null())
        //     .col(ColumnDef::new(Exchanges::Conversation).integer().not_null())
        //     .foreign_key(ForeignKey::create()
        //         .from(Exchanges::Table, Exchanges::Conversation)
        //         .to(Conversations::Table, Conversations::Id))
        //     .to_owned();
        // let _: Vec<_> = exchanges.into();
        // manager.create_table(exchanges).await?;

        // raw sql schema because "deferrable initially deferred
        // this can't be added later because of:
        // https://stackoverflow.com/questions/42969127/add-constraint-to-existing-sqlite-table
        manager.get_connection().execute_unprepared("
            create table if not exists exchanges (
                id integer primary key autoincrement not null,
                key integer not null,
                user_message text not null,
                assistant_message text not null,
                conversation integer not null,
                foreign key (conversation) references conversations(id) on delete cascade deferrable initially deferred
            );
        ").await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager.drop_table(Table::drop().table(Conversations::Table).to_owned()).await?;
        manager.drop_table(Table::drop().table(Exchanges::Table).to_owned()).await
    }
}

#[derive(DeriveIden)]
enum Conversations {
    Table,
    Id,
    Uuid,
    LastUpdated,
    FirstExchange
}

#[derive(DeriveIden)]
enum Exchanges {
    Table,
    Id,
    // Key,
    // UserMessage,
    // AssistantMessage,
    // Conversation
}