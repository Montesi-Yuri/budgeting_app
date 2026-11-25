// Runs Vitest from the canonically-cased working directory.
// On Windows a shell may report the cwd as "c:\..." while Node resolves
// modules as "C:\...", which makes Vitest load itself twice and fail with
// "Cannot read properties of undefined (reading 'config')".
import { realpathSync } from 'node:fs'
import { pathToFileURL } from 'node:url'

const cwd = realpathSync.native(process.cwd())
process.chdir(cwd.charAt(1) === ':' ? cwd.charAt(0).toUpperCase() + cwd.slice(1) : cwd)

await import(pathToFileURL(process.cwd() + '/node_modules/vitest/vitest.mjs').href)
