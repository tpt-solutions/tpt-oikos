package core

import "github.com/PhillipC05/tpt-identity/pkg/schema"

func init() {
	schema.RegisterCategory(schema.Category{
		ID:          "mandate",
		Name:        "Mandate & Delegation",
		Description: "Agent authority mandates and delegation credentials",
		Icon:        "shield-check",
	})

	for _, s := range []schema.Schema{
		{ID: "mandate.authority", CategoryID: "mandate", Name: "Mandate Authority", Source: schema.SourceCore,
			Claims: []schema.ClaimDefinition{
				{Name: "grantorDID", Type: "string", Required: true},
				{Name: "agentDID", Type: "string", Required: true},
				{Name: "scope", Type: "string", Required: true, Description: "Comma-separated permitted actions"},
				{Name: "validFrom", Type: "string", Required: true},
				{Name: "validUntil", Type: "string"},
				{Name: "maxBudgetKoin", Type: "string"},
				{Name: "allowedContracts", Type: "string", Description: "Comma-separated contract addresses"},
			}},
		{ID: "mandate.delegation", CategoryID: "mandate", Name: "Delegation Chain", Source: schema.SourceCore,
			Claims: []schema.ClaimDefinition{
				{Name: "parentMandateID", Type: "string", Required: true},
				{Name: "delegatorDID", Type: "string", Required: true},
				{Name: "delegateDID", Type: "string", Required: true},
				{Name: "scope", Type: "string", Required: true},
				{Name: "validFrom", Type: "string", Required: true},
				{Name: "validUntil", Type: "string"},
			}},
	} {
		schema.RegisterSchema(s)
	}
}
