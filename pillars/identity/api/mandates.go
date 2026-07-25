package api

import (
	"encoding/json"
	"net/http"
	"time"

	"github.com/google/uuid"
	"github.com/PhillipC05/tpt-identity/internal/events"
	"github.com/PhillipC05/tpt-identity/internal/store"
)

type createMandateRequest struct {
	GrantorDID       string     `json:"grantorDid"`
	AgentDID         string     `json:"agentDid"`
	SchemaID         string     `json:"schemaId"`
	Scope            string     `json:"scope"`
	ExpiresAt        *time.Time `json:"expiresAt,omitempty"`
	MaxBudgetKoin    string     `json:"maxBudgetKoin,omitempty"`
	AllowedContracts string     `json:"allowedContracts,omitempty"`
	TenantID         string     `json:"tenantId,omitempty"`
}

func (s *Server) handleListMandates(w http.ResponseWriter, r *http.Request) {
	grantorDID := r.URL.Query().Get("grantor")
	agentDID := r.URL.Query().Get("agent")

	var mandates []*store.Mandate
	var err error

	switch {
	case grantorDID != "":
		mandates, err = s.store.ListMandatesByGrantor(r.Context(), grantorDID)
	case agentDID != "":
		mandates, err = s.store.ListMandatesByAgent(r.Context(), agentDID)
	default:
		mandates, err = s.store.ListMandates(r.Context(), "")
	}
	if err != nil {
		http.Error(w, "list mandates: "+err.Error(), http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(mandates)
}

func (s *Server) handleCreateMandate(w http.ResponseWriter, r *http.Request) {
	var req createMandateRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, "bad request: "+err.Error(), http.StatusBadRequest)
		return
	}
	if req.GrantorDID == "" || req.AgentDID == "" || req.Scope == "" {
		http.Error(w, "grantorDid, agentDid, and scope are required", http.StatusBadRequest)
		return
	}

	now := time.Now()
	m := &store.Mandate{
		ID:               uuid.New().String(),
		GrantorDID:       req.GrantorDID,
		AgentDID:         req.AgentDID,
		SchemaID:         req.SchemaID,
		Scope:            req.Scope,
		Status:           "active",
		GrantedAt:        now,
		ExpiresAt:        req.ExpiresAt,
		MaxBudgetKoin:    req.MaxBudgetKoin,
		AllowedContracts: req.AllowedContracts,
		TenantID:         req.TenantID,
		CreatedAt:        now,
		UpdatedAt:        now,
	}

	if m.SchemaID == "" {
		m.SchemaID = "mandate.authority"
	}

	if err := s.store.SaveMandate(r.Context(), m); err != nil {
		http.Error(w, "save mandate: "+err.Error(), http.StatusInternalServerError)
		return
	}

	s.events.Publish(r.Context(), events.MandateCreated, m)

	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusCreated)
	json.NewEncoder(w).Encode(m)
}

func (s *Server) handleGetMandate(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	m, err := s.store.GetMandate(r.Context(), id)
	if err != nil {
		http.Error(w, "get mandate: "+err.Error(), http.StatusNotFound)
		return
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(m)
}

func (s *Server) handleRevokeMandate(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	now := time.Now()
	if err := s.store.UpdateMandateStatus(r.Context(), id, "revoked", &now); err != nil {
		http.Error(w, "revoke mandate: "+err.Error(), http.StatusInternalServerError)
		return
	}

	m, _ := s.store.GetMandate(r.Context(), id)
	if m != nil {
		s.events.Publish(r.Context(), events.MandateRevoked, m)
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(map[string]string{"status": "revoked"})
}

func (s *Server) handleDeleteMandate(w http.ResponseWriter, r *http.Request) {
	id := r.PathValue("id")
	if err := s.store.DeleteMandate(r.Context(), id); err != nil {
		http.Error(w, "delete mandate: "+err.Error(), http.StatusInternalServerError)
		return
	}
	w.WriteHeader(http.StatusNoContent)
}
