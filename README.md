# Canopy

Canopy is an experimental tool for creating temporary integration branches from labeled pull requests.

For example, adding the label `canopy/alpha` to a PR will include it in the generated `canopy-alpha` branch. Canopy will keep that generated branch up to date with the latest changes from main and from each PR labeled with the same label.

## Motivations

- Test unmerged changes with a small slice of real traffic before you approve them
- Combine several PRs into one temporary branch for canaries, or previews
- Try risky ideas or AI-generated code behind quickly without fullly shipping them
- Stop the experiment instantly just by removing the label
