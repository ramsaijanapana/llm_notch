export type NavLink = {
  label: string
  href: string
}

export type CtaLink = {
  label: string
  href: string
}

export type WorkflowStep = {
  step: string
  title: string
  description: string
}

export type CapabilityCard = {
  id: string
  microLabel: string
  title: string
  description: string
  layout: 'wide' | 'tall' | 'default'
}

export type FaqItem = {
  id: string
  question: string
  answer: string
}

export type SiteContent = {
  meta: {
    productName: string
    tagline: string
  }
  header: {
    nav: NavLink[]
    cta: CtaLink
  }
  hero: {
    eyebrow: string
    title: string
    description: string
    primaryCta: CtaLink
    secondaryCta: CtaLink
    trustLine: string
    coordinate: string
  }
  workflow: {
    id: string
    eyebrow: string
    title: string
    lead: string
    steps: WorkflowStep[]
    coordinate: string
  }
  capabilities: {
    id: string
    eyebrow: string
    title: string
    lead: string
    cards: CapabilityCard[]
    coordinate: string
  }
  localFirst: {
    id: string
    eyebrow: string
    title: string
    paragraphs: string[]
    bullets: string[]
    coordinate: string
  }
  pricing: {
    id: string
    eyebrow: string
    title: string
    subtitle: string
    note: string
    disclosure: string
    cta: CtaLink
    coordinate: string
  }
  faq: {
    id: string
    eyebrow: string
    title: string
    items: FaqItem[]
    coordinate: string
  }
  finalCta: {
    title: string
    description: string
    cta: CtaLink
    coordinate: string
  }
  footer: {
    statement: string
    nav: NavLink[]
    prototypeNote: string
  }
}

export const siteContent: SiteContent = {
  meta: {
    productName: 'llm_notch',
    tagline: 'Peripheral vision for your agents.',
  },
  header: {
    nav: [
      { label: 'Demo', href: '#demo' },
      { label: 'Workflow', href: '#workflow' },
      { label: 'Capabilities', href: '#capabilities' },
      { label: 'Local-first', href: '#local-first' },
      { label: 'FAQ', href: '#faq' },
    ],
    cta: { label: 'Play the demo', href: '#demo' },
  },
  hero: {
    eyebrow: 'MULTI-AGENT AWARENESS LAYER',
    title: 'Peripheral vision for your agents.',
    description:
      'A calm command surface for tracking several coding agents at once — without leaving your editor, terminal, or train of thought. Glance, triage, and step in only when it matters.',
    primaryCta: { label: 'Play the demo', href: '#demo' },
    secondaryCta: { label: 'Why local-first', href: '#local-first' },
    trustLine: 'Live readout below is simulated · no agents connected',
    coordinate: '0,0',
  },
  workflow: {
    id: 'workflow',
    eyebrow: 'PLANNED WORKFLOW',
    title: 'Attach, glance, intervene.',
    lead: 'A lightweight loop that keeps you in flow while agents run in parallel — attach, orient in seconds, intervene when it matters.',
    steps: [
      {
        step: '01',
        title: 'Attach',
        description:
          'Point llm_notch at the agent sessions you care about. Planned connectors for local CLIs and IDE extensions — scoped read access, no remote relay.',
      },
      {
        step: '02',
        title: 'Glance',
        description:
          'A peripheral strip surfaces status, stalls, and decisions that need you. Enough signal to orient in seconds, not enough noise to pull you out of deep work.',
      },
      {
        step: '03',
        title: 'Intervene',
        description:
          'Approve, redirect, or pause from the notch when an agent drifts. Intervention stays deliberate — you choose when to break focus.',
      },
    ],
    coordinate: '1,0',
  },
  capabilities: {
    id: 'capabilities',
    eyebrow: 'PRODUCT DIRECTION',
    title: 'Built for parallel attention.',
    lead: 'Designed for parallel attention — ranked signals, safe gates, and recall without leaving your workspace.',
    cards: [
      {
        id: 'attention-queue',
        microLabel: 'ATTN',
        title: 'Attention queue',
        description:
          'Ranked queue of agent events that actually need human judgment — stalled tasks, ambiguous diffs, blocked commands.',
        layout: 'wide',
      },
      {
        id: 'context-snapshots',
        microLabel: 'CTX',
        title: 'Context snapshots',
        description:
          'Frozen slices of repo state, last prompt, and tool output so you can decide without reopening five panes.',
        layout: 'default',
      },
      {
        id: 'safe-decisions',
        microLabel: 'SAFE',
        title: 'Safe decisions',
        description:
          'Explicit approve / defer gates for destructive or high-impact actions before an agent proceeds.',
        layout: 'default',
      },
      {
        id: 'usage-pulse',
        microLabel: 'PULSE',
        title: 'Usage pulse',
        description:
          'Token and runtime cadence at the edge of vision — spot runaway loops early without a separate dashboard.',
        layout: 'tall',
      },
      {
        id: 'terminal-recall',
        microLabel: 'RECALL',
        title: 'Terminal recall',
        description:
          'Scrollback anchors tied to agent turns. Jump to the command that changed the outcome, not the whole session log.',
        layout: 'default',
      },
    ],
    coordinate: '2,0',
  },
  localFirst: {
    id: 'local-first',
    eyebrow: 'LOCAL-FIRST BY DESIGN',
    title: 'Your agents stay on your machine.',
    paragraphs: [
      'This marketing page is fully static: it makes zero runtime network requests. What you load is what runs — no analytics beacon, no font CDN, no background sync.',
      'llm_notch is being designed as a local command surface. Agent traffic, file context, and decision history are intended to remain on-device. Integrations described elsewhere on this page are planned design goals, not shipped guarantees.',
    ],
    bullets: [
      'Static prototype · no data leaves this page',
      'Planned local connectors · no cloud relay required',
      'You control what sessions are attached and when',
    ],
    coordinate: '0,1',
  },
  pricing: {
    id: 'pricing',
    eyebrow: 'LAUNCH MODEL PREVIEW',
    title: 'One license. No subscription.',
    subtitle: 'Pay once, keep the tool. No recurring seat fees or usage tiers.',
    note: 'One-time purchase planned · price announced at launch.',
    disclosure: 'Subject to change before release. No payment is collected.',
    cta: { label: 'Try the demo first', href: '#demo' },
    coordinate: '1,1',
  },
  faq: {
    id: 'faq',
    eyebrow: 'FAQ',
    title: 'Practical questions.',
    items: [
      {
        id: 'concept-status',
        question: 'Is llm_notch shipping today?',
        answer:
          'No. This site is a concept prototype. The interactive demo simulates agent telemetry on your device. There is no downloadable binary, checkout flow, or account system on this page.',
      },
      {
        id: 'privacy',
        question: 'Does this page collect or send data?',
        answer:
          'No. The static layer performs no runtime network requests. The demo runs entirely in your browser with fabricated agent states. A future local app would be designed to keep session data on your machine.',
      },
      {
        id: 'supported-agents',
        question: 'Which agents will it support?',
        answer:
          'The product direction targets common local coding agents and CLIs — Cursor, Claude Code, Codex-style tools, and custom scripts you run in a terminal. Specific connectors are planned; none are wired in this prototype.',
      },
      {
        id: 'simulated-demo',
        question: 'What does the demo actually do?',
        answer:
          'It runs an interactive simulation in your browser: browse sessions, approve or reject actions, submit answers, control playback, and open the in-frame terminal. Nothing connects to a real agent runtime or leaves your device.',
      },
      {
        id: 'availability',
        question: 'When can I buy or download it?',
        answer:
          'Not yet. Launch timing, pricing, and packaging will be announced when there is a real build to evaluate. For now, use the demo to react to the interaction model.',
      },
    ],
    coordinate: '2,1',
  },
  finalCta: {
    title: 'See the notch in motion.',
    description:
      'Walk through the simulated readout and imagine glancing across three agents without breaking focus.',
    cta: { label: 'Play the demo', href: '#demo' },
    coordinate: '0,2',
  },
  footer: {
    statement: 'llm_notch — a calm, local-first command surface for multi-agent coding awareness.',
    nav: [
      { label: 'Demo', href: '#demo' },
      { label: 'Workflow', href: '#workflow' },
      { label: 'Capabilities', href: '#capabilities' },
      { label: 'Local-first', href: '#local-first' },
      { label: 'FAQ', href: '#faq' },
    ],
    prototypeNote: 'Concept prototype · no data leaves this page',
  },
}
