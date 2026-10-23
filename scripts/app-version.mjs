import { readFileSync } from 'node:fs'

const pkg = JSON.parse(readFileSync('package.json', 'utf8')).version
const tauri = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version
const cargo = readFileSync('src-tauri/Cargo.toml', 'utf8').match(/^version = "([^"]+)"/m)?.[1]

if (!/^\d+\.\d+\.\d+$/.test(pkg || '') || pkg !== tauri || pkg !== cargo) {
  console.error('Version phải giống nhau và đúng dạng x.y.z trong package.json, src-tauri/tauri.conf.json và src-tauri/Cargo.toml.')
  process.exit(1)
}

console.log(pkg)
