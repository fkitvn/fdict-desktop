import { readFileSync, writeFileSync } from 'node:fs'

const version = process.argv[2]
if (!/^\d+\.\d+\.\d+$/.test(version || '')) {
  console.error('Dùng: yarn version:set 1.0.4')
  process.exit(1)
}

const pkg = 'package.json'
const tauri = 'src-tauri/tauri.conf.json'
const cargo = 'src-tauri/Cargo.toml'

writeFileSync(pkg, readFileSync(pkg, 'utf8').replace(/"version": "[^"]+"/, `"version": "${version}"`))
writeFileSync(tauri, readFileSync(tauri, 'utf8').replace(/"version": "[^"]+"/, `"version": "${version}"`))
writeFileSync(cargo, readFileSync(cargo, 'utf8').replace(/^version = "[^"]+"/m, `version = "${version}"`))
console.log(version)
