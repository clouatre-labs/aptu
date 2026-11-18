# Architecture Diagrams

This repository documents its architecture with diagrams kept under `docs/diagrams/`.

## Convention

- Diagrams are Mermaid. The source is committed as a `.mmd` file alongside the rendered `.svg` so the diagram stays editable.
- Name each pair after the view it captures, for example `architecture.mmd` and `architecture.svg`.
- Reference diagrams from `README.md` so new contributors find the current architecture view on arrival.
- Re-render the `.svg` in the same change as any source edit; never commit a rendered image without its source.

Render with mmdc (neutral theme, scale 1), then strip the white background from the SVG before committing so it renders cleanly on dark themes:

```bash
cd docs/diagrams
PUPPETEER_EXECUTABLE_PATH="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  mmdc -i architecture.mmd -o architecture.svg -s 1 -t neutral
sed -i.bak 's/background-color: white;//g' architecture.svg && rm architecture.svg.bak
```

Keep node labels to one to three words so rendered diagrams stay legible at small sizes.
