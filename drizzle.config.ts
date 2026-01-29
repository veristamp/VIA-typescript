import { defineConfig } from 'drizzle-kit';
import { settings } from './src/config/settings.ts';

export default defineConfig({
  schema: './src/db/schema.ts',
  out: './src/db/migrations',
  dialect: 'postgresql',
  dbCredentials: {
    host: settings.postgres.host,
    port: settings.postgres.port,
    database: settings.postgres.database,
    user: settings.postgres.user,
    password: settings.postgres.password,
    ssl: false,
  },
});
