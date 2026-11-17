<script setup lang="ts">
import { computed, onMounted, ref } from 'vue'
import { useBackend } from '../backend'
import { failureMessage } from '../failure'
import { recoveryPath, recoverySentence } from '../radio-words'
import type { Assistant } from '../types/generated/Assistant'
import type { Invite } from '../types/generated/Invite'
import type { Joined } from '../types/generated/Joined'
import type { LinkAddress } from '../types/generated/LinkAddress'
import type { SaveOptions } from '../types/generated/SaveOptions'

const props = defineProps<{ assistant: Assistant }>()
const backend = useBackend()
const options = ref<SaveOptions>()
const loading = ref(true)
const action = ref<'' | 'address' | 'saveAddress' | 'invite' | 'join' | 'copy'>('')
const busy = computed(() => loading.value || !!action.value)
const error = ref('')
const copied = ref(false)
const address = ref<LinkAddress>()
const editingAddress = ref(false)
const addressField = ref('')
const computerName = ref('')
const nameTyped = ref(false)
const code = ref('')
const invitation = ref<Invite>()
const recoveryFile = ref<string | null>(null)
const joined = ref<Joined>()
const builder = computed(() => options.value?.memberComputer === null)
const hasAddress = computed(() => !!address.value?.addresses.length)
const expiry = computed(() => {
  const expiresAt = invitation.value?.expiresAt
  return typeof expiresAt === 'number' && Number.isFinite(expiresAt) && expiresAt > 0
    ? `It works once, until ${new Date(expiresAt * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}.`
    : 'It works once, for a short while.'
})
function clearMessages() {
  error.value = ''
  copied.value = false
}
async function load() {
  if (action.value) return
  loading.value = true
  clearMessages()
  try {
    options.value = await backend.saveOptions({ slug: props.assistant.slug })
  } catch (failure) {
    error.value = failureMessage(failure, `Voice couldn't check ${props.assistant.name}. Try again.`)
  } finally { loading.value = false }
}
onMounted(load)
function keepAddress(answer: LinkAddress) {
  address.value = answer
  if (!nameTyped.value) computerName.value = answer.computerName ?? ''
  editingAddress.value = false
}
async function readAddress() {
  if (busy.value) return
  action.value = 'address'
  clearMessages()
  try {
    keepAddress(await backend.radioLinkAddress())
  } catch (failure) {
    error.value = failureMessage(failure, "Voice couldn't read this computer's VPN address. Try again.")
  } finally { action.value = '' }
}
function changeAddress() {
  if (busy.value) return
  clearMessages()
  editingAddress.value = true
}
async function saveAddress() {
  if (busy.value || !addressField.value.trim()) return
  action.value = 'saveAddress'
  clearMessages()
  try {
    keepAddress(await backend.radioSetLinkAddress({ address: addressField.value.trim() }))
    invitation.value = undefined
    copied.value = false
  } catch (failure) {
    error.value = failureMessage(failure, "Voice couldn't save that address. Try again.")
  } finally { action.value = '' }
}
async function makeCode() {
  if (busy.value || !hasAddress.value || !builder.value) return
  action.value = 'invite'
  clearMessages()
  try {
    invitation.value = await backend.radioInvite({ slug: props.assistant.slug })
    if (invitation.value.recoveryFile !== null) recoveryFile.value = invitation.value.recoveryFile
  } catch (failure) {
    error.value = failureMessage(failure, "Voice couldn't make a join code. Try again.")
    const path = recoveryPath(error.value)
    if (path !== null) {
      recoveryFile.value = path
      error.value = error.value.replace(recoverySentence(path), '').trim()
    }
  } finally { action.value = '' }
}
async function copy() {
  if (busy.value || !invitation.value) return
  action.value = 'copy'
  clearMessages()
  try {
    await navigator.clipboard.writeText(invitation.value.code)
    copied.value = true
  } catch {
    error.value = "Voice couldn't copy the code. Select it and copy it yourself."
  } finally { action.value = '' }
}
async function join() {
  if (busy.value || !hasAddress.value || !computerName.value.trim() || !code.value.trim() || builder.value) return
  action.value = 'join'
  clearMessages()
  try {
    joined.value = await backend.radioJoin({ slug: props.assistant.slug, code: code.value.trim(), name: computerName.value.trim() })
    code.value = ''
  } catch (failure) {
    error.value = failureMessage(failure, "Voice couldn't join the other computer. Try again.")
  } finally { action.value = '' }
}
</script>

<template>
  <section class="card mt-4">
    <div class="card-body p-5">
      <h2 class="mb-3">Your other computers</h2>
      <div v-if="loading" class="spinner-border" role="status"><span class="visually-hidden">Checking {{ assistant.name }}…</span></div>
      <template v-else-if="!options">
        <div class="alert alert-warning" role="alert">{{ error }}</div>
        <button type="button" class="btn btn-primary" :disabled="busy" @click="load">Try again</button>
      </template>
      <template v-else>
        <p v-if="builder">Link another computer to this one. For now this only links them: {{ assistant.name }} doesn't learn from the other computer yet. Both computers need to be on your VPN, and the other one needs {{ assistant.name }} first: there, choose Make a new assistant, then Open one you saved on GitHub.</p>
        <p v-else>Link this computer to the one that builds {{ assistant.name }}. For now this only links them: {{ assistant.name }} doesn't learn from this computer yet. Both computers need to be on your VPN.</p>
        <div v-if="error" class="alert alert-warning" role="alert">{{ error }}</div>
        <button v-if="!address" type="button" class="btn btn-outline-primary" :disabled="busy" @click="readAddress">{{ action === 'address' ? 'Checking…' : builder ? 'Add another computer' : 'Link this computer' }}</button>
        <template v-else>
          <template v-if="hasAddress">
            <p>This computer's VPN address: {{ address.addresses.join(', ') }} (port {{ address.port }}).</p>
            <button type="button" class="btn btn-link p-0" :disabled="busy" @click="changeAddress">Change it</button>
          </template>
          <p v-else>This computer has no VPN address yet.</p>
          <div v-if="!hasAddress || editingAddress" class="mb-3">
            <label class="form-label" for="vpn-address">This computer's VPN address</label>
            <input id="vpn-address" v-model="addressField" class="form-control" inputmode="decimal" autocomplete="off" spellcheck="false" :disabled="busy">
            <div class="form-hint">The address your VPN gives this computer, such as 10.0.0.5.</div>
            <button type="button" class="btn btn-primary" :disabled="busy || !addressField.trim()" @click="saveAddress">{{ action === 'saveAddress' ? 'Saving…' : 'Save address' }}</button>
          </div>
          <template v-if="builder">
            <div v-if="hasAddress" class="btn-list">
              <button type="button" class="btn btn-primary" :disabled="busy" @click="makeCode">{{ action === 'invite' ? 'Making it…' : invitation ? 'Make a new code' : 'Make a join code' }}</button>
            </div>
            <template v-if="invitation">
              <div id="join-code-label" class="form-label">Join code</div>
              <code id="join-code-shown" aria-labelledby="join-code-label" class="d-block user-select-all text-break">{{ invitation.code }}</code>
              <button type="button" class="btn btn-outline-secondary" :disabled="busy" @click="copy">{{ copied ? 'Copied' : 'Copy' }}</button>
              <span v-if="copied" class="visually-hidden" role="status">Copied the join code.</span>
              <p>{{ expiry }}</p>
              <p>On the other computer, open {{ assistant.name }}'s Save step, choose Link this computer under Your other computers, paste this code, and choose Join.</p>
            </template>
          </template>
          <template v-else-if="!joined">
            <div class="mb-3">
              <label class="form-label" for="computer-name">A name for this computer</label>
              <input id="computer-name" v-model="computerName" class="form-control" maxlength="64" :disabled="busy" @input="nameTyped = true">
              <div class="form-hint">Your other computers show it by this name.</div>
            </div>
            <div class="mb-3">
              <label class="form-label" for="join-code">Join code</label>
              <input id="join-code" v-model="code" type="text" class="form-control" autocomplete="off" autocapitalize="off" spellcheck="false" :disabled="busy">
              <div class="form-hint">Make one on the computer that builds {{ assistant.name }}, on its Save step, then paste it here.</div>
            </div>
            <button type="button" class="btn btn-primary" :disabled="busy || !hasAddress || !computerName.trim() || !code.trim()" @click="join">{{ action === 'join' ? 'Joining…' : 'Join' }}</button>
          </template>
          <div v-else class="alert alert-success" role="status">This computer joined as {{ joined.computerName }}.</div>
        </template>
        <div v-if="recoveryFile !== null" class="alert alert-warning" role="alert">{{ recoverySentence(recoveryFile) }}</div>
      </template>
    </div>
  </section>
</template>
