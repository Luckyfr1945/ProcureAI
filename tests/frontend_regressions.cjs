const { test } = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
function context(document) {
  const scope = vm.createContext({ document, URL, console, setTimeout, clearTimeout, clearInterval });
  vm.runInContext(fs.readFileSync('frontend/js/app.js', 'utf8'), scope);
  return scope;
}
test('contact content remains data, never an inline JavaScript handler', async () => {
  const scope = context({ createElement(tag) { return { tag, listeners: {}, addEventListener(name, fn) { this.listeners[name] = fn; } }; } });
  const container = { children: [], replaceChildren() { this.children=[]; }, append(...nodes) { this.children.push(...nodes); } };
  scope.container = container;
  scope.payload = "x');globalThis.injected=true;//";
  vm.runInContext("globalThis.copied=null; copyText = value => { globalThis.copied=value; }; renderContactValue(container,payload,'Email');", scope);
  assert.equal(container.children[0].textContent, scope.payload);
  const button = container.children.find(node => node?.tag === 'button');
  assert.equal(button.onclick, undefined);
  button.listeners.click();
  assert.equal(scope.copied, scope.payload);
  assert.equal(scope.injected, undefined);
});
test('late status response cannot modify the active search', async () => {
  let writes = 0;
  const scope = context({ getElementById() { writes++; throw new Error('stale DOM write'); } });
  vm.runInContext("currentJobId='new'; API.request=async()=>({ job_id:'old', status:'completed' });", scope);
  await vm.runInContext("pollJobStatus('old')", scope);
  assert.equal(writes, 0);
});
test('browser tokens are removed instead of persisted in localStorage', () => {
  const values = new Map([['procure_token','old-token']]);
  const scope = context({});
  scope.localStorage = { getItem: key => values.get(key), removeItem: key => values.delete(key) };
  vm.runInContext("API.setToken('new-token')", scope);
  assert.equal(values.has('procure_token'), false);
  assert.equal(vm.runInContext('API.getToken()', scope), null);
});
