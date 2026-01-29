# VIA v2 (Bun Runtime)

VeriStamp Incident Atlas v2 - Built with Bun, Hono, and Drizzle ORM.

## Quick Start

```bash
# Install dependencies
bun install

# Run migrations
bun run db:migrate

# Start development server
bun run dev
```

## Project Structure

```
src/
├── algorithms/     # EWMA, HyperLogLog, CUSUM implementations
├── core/           # Domain models (AnomalyProfile, StateManager)
├── services/       # Business logic (Tier1Engine, IngestionService)
├── db/             # Drizzle schema and registry client
├── api/            # Hono routes and middleware
├── queue/          # AsyncQueue and QueueWorker
├── config/         # JSON5 configuration files
└── main.ts         # Application entry point
```

## Commands

| Command | Description |
|---------|-------------|
| `bun run dev` | Start dev server with hot reload |
| `bun run build` | Build for production |
| `bun run compile` | Compile to single binary |
| `bun run db:generate` | Generate migrations |
| `bun run db:migrate` | Run migrations |
| `bun run db:studio` | Open Drizzle Studio |
| `bun run test` | Run tests |
| `bun run lint` | Run Biome linter |
