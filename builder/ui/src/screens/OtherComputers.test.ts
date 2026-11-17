import { flushPromises } from '@vue/test-utils'
import { afterEach, expect, it, vi } from 'vitest'
import { assistant, button, deferred, fakeBackend, saveOptions, screen } from '../test-support'
import type { Backend } from '../backend'
import type { Invite } from '../types/generated/Invite'
import type { Joined } from '../types/generated/Joined'
import OtherComputers from './OtherComputers.vue'
import * as radioWords from '../radio-words'

const address = { port: 7341, addresses: ['10.0.0.5'], computerName: 'Studio desktop' }
const invite = { code: 'rj1.example', expiresAt: 1800000000, port: 7341, addresses: ['10.0.0.5'], recoveryFile: null }
const recovery = 'Radio saved a recovery key for your computers in ~/example/radio-recovery.key. Move it into your password manager, then delete that file.'
const memberOptions = { ...saveOptions, memberComputer: 'Studio desktop saves Example; this computer takes each new version.' }
async function ready(overrides: Partial<Backend> = {}, member = false) {
  const backend = fakeBackend({ saveOptions: vi.fn().mockResolvedValue(member ? memberOptions : saveOptions), ...overrides })
  const wrapper = screen(OtherComputers, backend, { assistant })
  await flushPromises()
  return { backend, wrapper }
}
async function linked(overrides: Partial<Backend> = {}, member = false) {
  const result = await ready({ radioLinkAddress: vi.fn().mockResolvedValue(address), ...overrides }, member)
  await button(result.wrapper, member ? 'Link this computer' : 'Add another computer').trigger('click')
  await flushPromises()
  return result
}
afterEach(() => vi.unstubAllGlobals())

it('loads only the role, announces loading, and leaves Radio alone', async () => {
  const pending = deferred<typeof saveOptions>()
  const { wrapper, backend } = await ready({ saveOptions: vi.fn().mockReturnValue(pending.promise) })
  expect(wrapper.get('[role="status"]').text()).toBe('Checking Example…')
  expect(backend.saveOptions).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  for (const method of ['radioLinkAddress', 'radioSetLinkAddress', 'radioInvite', 'radioJoin', 'radioName'] as const) expect(backend[method]).not.toHaveBeenCalled()
  pending.resolve(saveOptions); await flushPromises()
  expect(button(wrapper, 'Add another computer').exists()).toBe(true)
  expect(wrapper.findAll('button').some(b => b.text() === 'Join')).toBe(false)
})
it.each([{ message: 'Please try again.' }, new Error('private detail')])('retries role failures safely: %s', async failure => {
  const { wrapper, backend } = await ready({ saveOptions: vi.fn().mockRejectedValueOnce(failure).mockResolvedValue(saveOptions) })
  expect(wrapper.get('[role="alert"]').text()).toBe(failure instanceof Error ? "Voice couldn't check Example. Try again." : failure.message)
  expect(wrapper.text()).not.toContain('private detail')
  await button(wrapper, 'Try again').trigger('click'); await flushPromises()
  expect(backend.saveOptions).toHaveBeenCalledTimes(2)
  expect(button(wrapper, 'Add another computer').exists()).toBe(true)
})
it('makes and replaces codes, locks all controls, and renders expiry and recovery', async () => {
  const pending = deferred<Invite>()
  const { wrapper, backend } = await linked({ radioInvite: vi.fn().mockReturnValueOnce(pending.promise).mockResolvedValue({ ...invite, code: 'rj1.replacement', expiresAt: 0 }) })
  expect(backend.radioLinkAddress).toHaveBeenCalledExactlyOnceWith()
  expect(wrapper.text()).toContain("This computer's VPN address: 10.0.0.5 (port 7341).")
  await button(wrapper, 'Make a join code').trigger('click')
  expect(backend.radioInvite).toHaveBeenCalledExactlyOnceWith({ slug: 'example' })
  expect(button(wrapper, 'Making it…').exists()).toBe(true)
  for (const control of wrapper.findAll('button, input')) expect(control.attributes('disabled')).toBeDefined()
  pending.resolve({ ...invite, recoveryFile: '~/example/radio-recovery.key' }); await flushPromises()
  expect(wrapper.get('#join-code-shown').text()).toBe(invite.code)
  expect(wrapper.text()).toContain(`It works once, until ${new Date(invite.expiresAt * 1000).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}.`)
  expect(wrapper.get('[role="alert"]').text()).toBe(recovery)
  const { recoverySentence } = await import('../radio-words')
  expect(recoverySentence('~/example/radio-recovery.key')).toBe(recovery)
  for (const element of wrapper.findAll('*')) expect(Object.values(element.attributes()).join(' ')).not.toContain(invite.code)
  const writeText = vi.fn().mockResolvedValue(undefined)
  vi.stubGlobal('navigator', { clipboard: { writeText } })
  await button(wrapper, 'Copy').trigger('click'); await flushPromises()
  expect(writeText).toHaveBeenCalledExactlyOnceWith(invite.code)
  expect(button(wrapper, 'Copied').exists()).toBe(true)
  await button(wrapper, 'Make a new code').trigger('click'); await flushPromises()
  expect(wrapper.get('#join-code-shown').text()).toBe('rj1.replacement')
  expect(button(wrapper, 'Copy').exists()).toBe(true)
  expect(wrapper.text()).toContain('It works once, for a short while.')
  expect(wrapper.get('[role="alert"]').text()).toBe(recovery)
})
it.each([0, -1, NaN, Infinity])('uses the expiry fallback for %s', async expiresAt => {
  const { wrapper } = await linked({ radioInvite: vi.fn().mockResolvedValue({ ...invite, expiresAt }) })
  await button(wrapper, 'Make a join code').trigger('click'); await flushPromises()
  expect(wrapper.text()).toContain('It works once, for a short while.')
})
it.each([undefined, { writeText: vi.fn().mockRejectedValue(new Error('private detail')) }, { writeText: vi.fn().mockRejectedValue({ message: 'private detail' }) }])('handles unavailable or refusing clipboards', async clipboard => {
  vi.stubGlobal('navigator', { clipboard })
  const { wrapper } = await linked()
  await button(wrapper, 'Make a join code').trigger('click'); await flushPromises()
  await button(wrapper, 'Copy').trigger('click'); await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toBe("Voice couldn't copy the code. Select it and copy it yourself.")
})
it('keeps the entire invite refusal including its recovery sentence', async () => {
  const message = `Voice couldn't make a join code. ${recovery}`
  const { wrapper } = await linked({ radioInvite: vi.fn().mockRejectedValue({ message }) })
  await button(wrapper, 'Make a join code').trigger('click'); await flushPromises()
  expect(wrapper.findAll('[role="alert"]').map(alert => alert.text())).toContain(message)
})
it('sets an empty address, keeps a refused field, and changes an existing address', async () => {
  const { wrapper, backend } = await linked({ radioLinkAddress: vi.fn().mockResolvedValue({ ...address, addresses: [] }), radioSetLinkAddress: vi.fn().mockRejectedValueOnce({ message: 'Try that address again.' }).mockResolvedValue({ ...address, addresses: ['10.0.0.7'], port: 7342 }) })
  expect(wrapper.text()).toContain('This computer has no VPN address yet.')
  expect(button(wrapper, 'Save address').attributes('disabled')).toBeDefined()
  expect(wrapper.findAll('button').some(b => b.text() === 'Make a join code')).toBe(false)
  await wrapper.get('#vpn-address').setValue('  10.0.0.7 ')
  await button(wrapper, 'Save address').trigger('click'); await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toBe('Try that address again.')
  expect(wrapper.get<HTMLInputElement>('#vpn-address').element.value).toBe('  10.0.0.7 ')
  await button(wrapper, 'Save address').trigger('click'); await flushPromises()
  expect(backend.radioSetLinkAddress).toHaveBeenLastCalledWith({ address: '10.0.0.7' })
  expect(wrapper.text()).toContain("This computer's VPN address: 10.0.0.7 (port 7342).")
  expect(wrapper.find('#vpn-address').exists()).toBe(false)
  expect(wrapper.find('[role="alert"]').exists()).toBe(false)
  await button(wrapper, 'Change it').trigger('click')
  expect(wrapper.find('#vpn-address').exists()).toBe(true)
})
it('prefills once, preserves a typed name on address save, and joins with trimmed fields', async () => {
  const pending = deferred<Joined>()
  const { wrapper, backend } = await linked({ radioSetLinkAddress: vi.fn().mockResolvedValue({ ...address, computerName: 'Another guess' }), radioJoin: vi.fn().mockReturnValue(pending.promise) }, true)
  expect(wrapper.get<HTMLInputElement>('#computer-name').element.value).toBe('Studio desktop')
  expect(wrapper.findAll('button').some(b => b.text() === 'Make a join code')).toBe(false)
  expect(button(wrapper, 'Join').attributes('disabled')).toBeDefined()
  await wrapper.get('#computer-name').setValue('  ')
  await wrapper.get('#join-code').setValue(' rj1.example ')
  expect(button(wrapper, 'Join').attributes('disabled')).toBeDefined()
  await wrapper.get('#computer-name').setValue('  Work laptop ')
  expect(button(wrapper, 'Join').attributes('disabled')).toBeUndefined()
  await button(wrapper, 'Change it').trigger('click')
  await wrapper.get('#vpn-address').setValue('10.0.0.7')
  await button(wrapper, 'Save address').trigger('click'); await flushPromises()
  expect(wrapper.get<HTMLInputElement>('#computer-name').element.value).toBe('  Work laptop ')
  for (const element of wrapper.findAll('*')) expect(Object.values(element.attributes()).join(' ')).not.toContain('rj1.example')
  await button(wrapper, 'Join').trigger('click')
  expect(backend.radioJoin).toHaveBeenCalledExactlyOnceWith({ slug: 'example', code: 'rj1.example', name: 'Work laptop' })
  expect(button(wrapper, 'Joining…').exists()).toBe(true)
  for (const control of wrapper.findAll('button, input')) expect(control.attributes('disabled')).toBeDefined()
  pending.resolve({ fleet: 'voice', computerName: 'Work laptop' }); await flushPromises()
  expect(wrapper.get('[role="status"]').text()).toBe('This computer joined as Work laptop.')
  expect(wrapper.find('#join-code').exists()).toBe(false)
  expect(wrapper.html()).not.toContain('rj1.example')
  // The hidden field must also be cleared from component state.
  expect((wrapper.vm as unknown as { code: string }).code).toBe('')
})
it('keeps both fields and the complete join refusal', async () => {
  const message = 'The other computer turned that code down. It may be used or out of date: make a new one there, then try again.'
  const { wrapper } = await linked({ radioJoin: vi.fn().mockRejectedValue({ message }) }, true)
  await wrapper.get('#computer-name').setValue(' Work laptop ')
  await wrapper.get('#join-code').setValue(' rj1.example ')
  await button(wrapper, 'Join').trigger('click'); await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toBe(message)
  expect(wrapper.get<HTMLInputElement>('#computer-name').element.value).toBe(' Work laptop ')
  expect(wrapper.get<HTMLInputElement>('#join-code').element.value).toBe(' rj1.example ')
})
type Actions = {
  makeCode: () => Promise<void>; readAddress: () => Promise<void>; saveAddress: () => Promise<void>
  join: () => Promise<void>; copy: () => Promise<void>; changeAddress: () => void
  invitation: Invite; addressField: string
}
it.each([false, true])('guards every reachable action directly while busy (member: %s)', async member => {
  const pending = deferred<typeof address>()
  const writeText = vi.fn().mockResolvedValue(undefined)
  vi.stubGlobal('navigator', { clipboard: { writeText } })
  const { wrapper, backend } = await linked({ radioSetLinkAddress: vi.fn().mockReturnValue(pending.promise) }, member)
  const actions = wrapper.vm as unknown as Actions
  actions.invitation = invite
  if (member) await wrapper.get('#computer-name').setValue('Work laptop')
  if (member) await wrapper.get('#join-code').setValue('rj1.example')
  actions.addressField = '10.0.0.7'
  const first = actions.saveAddress()
  void actions.makeCode(); void actions.readAddress(); void actions.saveAddress(); void actions.join(); void actions.copy(); actions.changeAddress()
  await flushPromises()
  expect(backend.radioInvite).not.toHaveBeenCalled()
  expect(backend.radioLinkAddress).toHaveBeenCalledTimes(1)
  expect(backend.radioSetLinkAddress).toHaveBeenCalledTimes(1)
  expect(backend.radioJoin).not.toHaveBeenCalled()
  expect(writeText).not.toHaveBeenCalled()
  expect(wrapper.find('#vpn-address').exists()).toBe(false)
  pending.resolve(address); await first
})

it.each([
  ['radioLinkAddress', "Voice couldn't read this computer's VPN address. Try again."],
  ['radioSetLinkAddress', "Voice couldn't save that address. Try again."],
  ['radioInvite', "Voice couldn't make a join code. Try again."],
  ['radioJoin', "Voice couldn't join the other computer. Try again."],
] as const)('screens plain and raw failures for %s', async (method, fallback) => {
  for (const failure of [{ message: 'Please try that again.' }, new Error('private detail')]) {
    const overrides = { [method]: vi.fn().mockRejectedValue(failure) }
    const { wrapper } = method === 'radioLinkAddress' ? await ready(overrides) : await linked(overrides, method === 'radioJoin')
    if (method === 'radioSetLinkAddress') {
      await button(wrapper, 'Change it').trigger('click')
      await wrapper.get('#vpn-address').setValue('10.0.0.7')
    }
    if (method === 'radioJoin') await wrapper.get('#join-code').setValue('rj1.example')
    await button(wrapper, { radioLinkAddress: 'Add another computer', radioSetLinkAddress: 'Save address', radioInvite: 'Make a join code', radioJoin: 'Join' }[method]).trigger('click')
    await flushPromises()
    expect(wrapper.get('[role="alert"]').text()).toBe(failure instanceof Error ? fallback : failure.message)
    expect(wrapper.text()).not.toContain('private detail')
    wrapper.unmount()
  }
})

it('keeps a recovery path from a refusal after a later invite succeeds without one', async () => {
  const { wrapper } = await linked({ radioInvite: vi.fn().mockRejectedValueOnce({ message: `Try again. ${recovery}` }).mockResolvedValue(invite) })
  await button(wrapper, 'Make a join code').trigger('click'); await flushPromises()
  expect(wrapper.text()).toContain(`Try again. ${recovery}`)
  await button(wrapper, 'Make a join code').trigger('click'); await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toBe(recovery)
})
it('extracts only the recovery sentence, including literal punctuation in its template', () => {
  const { recoveryPath } = radioWords
  expect(recoveryPath('No recovery file was made.')).toBeNull()
  expect(recoveryPath(`Try again. ${recovery}`)).toBe('~/example/radio-recovery.key')
  expect(recoveryPath(radioWords.recoverySentence('~/example/a.b[1](key)+.key'))).toBe('~/example/a.b[1](key)+.key')
  expect(recoveryPath(recovery.replace('file.', 'file!'))).toBeNull()
})
it('labels the shown code and announces copying while locking every builder control', async () => {
  const pending = deferred<void>()
  vi.stubGlobal('navigator', { clipboard: { writeText: vi.fn().mockReturnValue(pending.promise) } })
  const { wrapper, backend } = await linked()
  await button(wrapper, 'Make a join code').trigger('click'); await flushPromises()
  expect(wrapper.get('#join-code-shown').attributes('aria-labelledby')).toBe('join-code-label')
  expect(wrapper.get('#join-code-label').element.tagName).toBe('DIV')
  await button(wrapper, 'Change it').trigger('click')
  await wrapper.get('#vpn-address').setValue('10.0.0.7')
  await button(wrapper, 'Copy').trigger('click')
  for (const control of wrapper.findAll('button, input')) expect(control.attributes('disabled'), control.text() || control.attributes('id')).toBeDefined()
  await button(wrapper, 'Make a new code').trigger('click')
  expect(backend.radioInvite).toHaveBeenCalledTimes(1)
  pending.resolve(); await flushPromises()
  expect(wrapper.get('[role="status"]').text()).toBe('Copied the join code.')
})
it('disables all member fields and buttons while an address save is pending', async () => {
  const pending = deferred<typeof address>()
  const { wrapper } = await linked({ radioSetLinkAddress: vi.fn().mockReturnValue(pending.promise) }, true)
  await wrapper.get('#join-code').setValue('rj1.example')
  await button(wrapper, 'Change it').trigger('click')
  await wrapper.get('#vpn-address').setValue('10.0.0.7')
  await button(wrapper, 'Save address').trigger('click')
  for (const control of wrapper.findAll('button, input')) expect(control.attributes('disabled'), control.text() || control.attributes('id')).toBeDefined()
  pending.resolve(address); await flushPromises()
})
it('clears a copied code after saving an address but keeps the recovery warning', async () => {
  vi.stubGlobal('navigator', { clipboard: { writeText: vi.fn().mockResolvedValue(undefined) } })
  const { wrapper } = await linked({ radioInvite: vi.fn().mockResolvedValue({ ...invite, recoveryFile: '~/example/radio-recovery.key' }) })
  await button(wrapper, 'Make a join code').trigger('click'); await flushPromises()
  await button(wrapper, 'Copy').trigger('click'); await flushPromises()
  await button(wrapper, 'Change it').trigger('click')
  await wrapper.get('#vpn-address').setValue('10.0.0.7')
  await button(wrapper, 'Save address').trigger('click'); await flushPromises()
  expect(wrapper.find('#join-code-shown').exists()).toBe(false)
  expect((wrapper.vm as unknown as { copied: boolean }).copied).toBe(false)
  expect(wrapper.get('[role="alert"]').text()).toBe(recovery)
})
it('keeps the builder out of the join form after reading an address', async () => {
  const { wrapper } = await linked()
  expect(wrapper.find('#join-code').exists()).toBe(false)
  expect(wrapper.findAll('button').some(b => b.text() === 'Join')).toBe(false)
})
it('requires an address even with both member fields filled', async () => {
  const { wrapper, backend } = await linked({ radioLinkAddress: vi.fn().mockResolvedValue({ ...address, addresses: [] }) }, true)
  await wrapper.get('#computer-name').setValue('Work laptop')
  await wrapper.get('#join-code').setValue('rj1.example')
  expect(button(wrapper, 'Join').attributes('disabled')).toBeDefined()
  const actions = wrapper.vm as unknown as Actions
  await actions.join()
  expect(backend.radioJoin).not.toHaveBeenCalled()
})
it('renders no empty button list without an address', async () => {
  const { wrapper } = await linked({ radioLinkAddress: vi.fn().mockResolvedValue({ ...address, addresses: [] }) })
  expect(wrapper.find('.btn-list').exists()).toBe(false)
})
it.each([false, true])('describes linking honestly with the complete steps (member: %s)', async member => {
  const { wrapper } = await linked({}, member)
  expect(wrapper.findAll('p')[0]?.text()).toBe(member
    ? "Link this computer to the one that builds Example. For now this only links them: Example doesn't learn from this computer yet. Both computers need to be on your VPN."
    : "Link another computer to this one. For now this only links them: Example doesn't learn from the other computer yet. Both computers need to be on your VPN, and the other one needs Example first: there, choose Make a new assistant, then Open one you saved on GitHub.")
  if (!member) {
    await button(wrapper, 'Make a join code').trigger('click'); await flushPromises()
    expect(wrapper.text()).toContain("On the other computer, open Example's Save step, choose Link this computer under Your other computers, paste this code, and choose Join.")
  }
})
