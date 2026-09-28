# figures/

Put images referenced by tips here. A tip refers to them relative to the
`tips/` directory:

```yaml
images:
  - figures/coil-thermal.png
```

or inline in the body:

```markdown
![Predicted hotspot](figures/coil-thermal.png)
```

The app fetches them through the GitHub API, so private repositories work.
Keep files under 1 MB; larger blobs are not served by the Contents endpoint.
