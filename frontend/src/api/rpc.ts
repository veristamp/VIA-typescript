import { hc } from "hono/client";
import type { app } from "../../../src/main";

// In development, we point to the backend port. 
// In production, we assume the backend serves the frontend from the same origin.
const client = hc<typeof app>(import.meta.env.VITE_API_URL || "http://localhost:8787");

export default client;
