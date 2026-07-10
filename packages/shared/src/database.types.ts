export type Json =
  | string
  | number
  | boolean
  | null
  | { [key: string]: Json | undefined }
  | Json[]

export type Database = {
  public: {
    Tables: {
      blocks: {
        Row: {
          blocked_id: string
          blocker_id: string
          created_at: string
        }
        Insert: {
          blocked_id: string
          blocker_id: string
          created_at?: string
        }
        Update: {
          blocked_id?: string
          blocker_id?: string
          created_at?: string
        }
        Relationships: [
          {
            foreignKeyName: "blocks_blocked_id_fkey"
            columns: ["blocked_id"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "blocks_blocker_id_fkey"
            columns: ["blocker_id"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
        ]
      }
      events: {
        Row: {
          created_at: string
          id: number
          kind: string
          occurred_at: string
          params: Json
          user_id: string
        }
        Insert: {
          created_at?: string
          id?: never
          kind: string
          occurred_at?: string
          params: Json
          user_id?: string
        }
        Update: {
          created_at?: string
          id?: never
          kind?: string
          occurred_at?: string
          params?: Json
          user_id?: string
        }
        Relationships: []
      }
      friend_mutes: {
        Row: {
          created_at: string
          muted: boolean
          muted_user_id: string
          owner_id: string
          updated_at: string
        }
        Insert: {
          created_at?: string
          muted?: boolean
          muted_user_id: string
          owner_id: string
          updated_at?: string
        }
        Update: {
          created_at?: string
          muted?: boolean
          muted_user_id?: string
          owner_id?: string
          updated_at?: string
        }
        Relationships: [
          {
            foreignKeyName: "friend_mutes_muted_user_id_fkey"
            columns: ["muted_user_id"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "friend_mutes_owner_id_fkey"
            columns: ["owner_id"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
        ]
      }
      friendships: {
        Row: {
          accepted_at: string | null
          created_at: string
          requester_id: string
          status: string
          updated_at: string
          user_a: string
          user_b: string
        }
        Insert: {
          accepted_at?: string | null
          created_at?: string
          requester_id: string
          status?: string
          updated_at?: string
          user_a: string
          user_b: string
        }
        Update: {
          accepted_at?: string | null
          created_at?: string
          requester_id?: string
          status?: string
          updated_at?: string
          user_a?: string
          user_b?: string
        }
        Relationships: [
          {
            foreignKeyName: "friendships_requester_id_fkey"
            columns: ["requester_id"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "friendships_user_a_fkey"
            columns: ["user_a"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "friendships_user_b_fkey"
            columns: ["user_b"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
        ]
      }
      invite_codes: {
        Row: {
          code_hash: string
          created_at: string
          expires_at: string
          id: string
          issued_by: string | null
          max_uses: number
          revoked_at: string | null
          use_count: number
        }
        Insert: {
          code_hash: string
          created_at?: string
          expires_at: string
          id?: string
          issued_by?: string | null
          max_uses?: number
          revoked_at?: string | null
          use_count?: number
        }
        Update: {
          code_hash?: string
          created_at?: string
          expires_at?: string
          id?: string
          issued_by?: string | null
          max_uses?: number
          revoked_at?: string | null
          use_count?: number
        }
        Relationships: [
          {
            foreignKeyName: "invite_codes_issued_by_fkey"
            columns: ["issued_by"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
        ]
      }
      invite_redemptions: {
        Row: {
          id: string
          invite_code_id: string
          redeemed_at: string
          user_id: string
        }
        Insert: {
          id?: string
          invite_code_id: string
          redeemed_at?: string
          user_id: string
        }
        Update: {
          id?: string
          invite_code_id?: string
          redeemed_at?: string
          user_id?: string
        }
        Relationships: [
          {
            foreignKeyName: "invite_redemptions_invite_code_id_fkey"
            columns: ["invite_code_id"]
            isOneToOne: false
            referencedRelation: "invite_codes"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "invite_redemptions_user_id_fkey"
            columns: ["user_id"]
            isOneToOne: true
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
        ]
      }
      pet_projections: {
        Row: {
          display_name: string
          owner_user_id: string
          pet_id: string
          skin_id: string
          status: Database["public"]["Enums"]["projection_status"]
          updated_at: string
          version: number
        }
        Insert: {
          display_name: string
          owner_user_id: string
          pet_id: string
          skin_id: string
          status: Database["public"]["Enums"]["projection_status"]
          updated_at?: string
          version?: number
        }
        Update: {
          display_name?: string
          owner_user_id?: string
          pet_id?: string
          skin_id?: string
          status?: Database["public"]["Enums"]["projection_status"]
          updated_at?: string
          version?: number
        }
        Relationships: [
          {
            foreignKeyName: "pet_projections_owner_user_id_fkey"
            columns: ["owner_user_id"]
            isOneToOne: true
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
        ]
      }
      pets: {
        Row: {
          connector_seen_at: string | null
          snapshot: Json
          updated_at: string
          user_id: string
        }
        Insert: {
          connector_seen_at?: string | null
          snapshot?: Json
          updated_at?: string
          user_id: string
        }
        Update: {
          connector_seen_at?: string | null
          snapshot?: Json
          updated_at?: string
          user_id?: string
        }
        Relationships: []
      }
      profiles: {
        Row: {
          avatar_url: string | null
          created_at: string
          display_name: string | null
          handle: string
          id: string
          updated_at: string
        }
        Insert: {
          avatar_url?: string | null
          created_at?: string
          display_name?: string | null
          handle: string
          id: string
          updated_at?: string
        }
        Update: {
          avatar_url?: string | null
          created_at?: string
          display_name?: string | null
          handle?: string
          id?: string
          updated_at?: string
        }
        Relationships: []
      }
      shared_memories: {
        Row: {
          created_at: string
          host_user_id: string
          id: string
          params: Json
          template_key: Database["public"]["Enums"]["memory_template_key"]
          visit_id: string
          visitor_user_id: string
        }
        Insert: {
          created_at?: string
          host_user_id: string
          id?: string
          params: Json
          template_key: Database["public"]["Enums"]["memory_template_key"]
          visit_id: string
          visitor_user_id: string
        }
        Update: {
          created_at?: string
          host_user_id?: string
          id?: string
          params?: Json
          template_key?: Database["public"]["Enums"]["memory_template_key"]
          visit_id?: string
          visitor_user_id?: string
        }
        Relationships: [
          {
            foreignKeyName: "shared_memories_host_user_id_fkey"
            columns: ["host_user_id"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "shared_memories_visit_id_fkey"
            columns: ["visit_id"]
            isOneToOne: true
            referencedRelation: "visits"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "shared_memories_visitor_user_id_fkey"
            columns: ["visitor_user_id"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
        ]
      }
      visits: {
        Row: {
          created_at: string
          ended_at: string | null
          ends_at: string | null
          host_user_id: string
          id: string
          request_expires_at: string
          requested_at: string
          returning_started_at: string | null
          started_at: string | null
          status: Database["public"]["Enums"]["visit_status"]
          terminal_status: Database["public"]["Enums"]["visit_status"] | null
          updated_at: string
          visitor_user_id: string
        }
        Insert: {
          created_at?: string
          ended_at?: string | null
          ends_at?: string | null
          host_user_id: string
          id?: string
          request_expires_at?: string
          requested_at?: string
          returning_started_at?: string | null
          started_at?: string | null
          status?: Database["public"]["Enums"]["visit_status"]
          terminal_status?: Database["public"]["Enums"]["visit_status"] | null
          updated_at?: string
          visitor_user_id: string
        }
        Update: {
          created_at?: string
          ended_at?: string | null
          ends_at?: string | null
          host_user_id?: string
          id?: string
          request_expires_at?: string
          requested_at?: string
          returning_started_at?: string | null
          started_at?: string | null
          status?: Database["public"]["Enums"]["visit_status"]
          terminal_status?: Database["public"]["Enums"]["visit_status"] | null
          updated_at?: string
          visitor_user_id?: string
        }
        Relationships: [
          {
            foreignKeyName: "visits_host_user_id_fkey"
            columns: ["host_user_id"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
          {
            foreignKeyName: "visits_visitor_user_id_fkey"
            columns: ["visitor_user_id"]
            isOneToOne: false
            referencedRelation: "profiles"
            referencedColumns: ["id"]
          },
        ]
      }
      waitlist: {
        Row: {
          created_at: string
          email: string
          id: number
        }
        Insert: {
          created_at?: string
          email: string
          id?: never
        }
        Update: {
          created_at?: string
          email?: string
          id?: never
        }
        Relationships: []
      }
    }
    Views: {
      funnel_friend_request_acceptance: {
        Row: {
          accepted_at: string | null
          converted: boolean | null
          pair_user_a: string | null
          pair_user_b: string | null
          request_sent_at: string | null
        }
        Relationships: []
      }
      funnel_friend_to_first_visit: {
        Row: {
          converted: boolean | null
          first_visit_requested_at: string | null
          friendship_accepted_at: string | null
          pair_user_a: string | null
          pair_user_b: string | null
        }
        Relationships: []
      }
      funnel_memory_view: {
        Row: {
          converted: boolean | null
          first_memory_viewed_at: string | null
          memory_id: string | null
          visit_completed_at: string | null
          visit_id: string | null
        }
        Relationships: []
      }
      funnel_seven_day_repeat_visit: {
        Row: {
          converted: boolean | null
          first_visit_completed_at: string | null
          first_visit_id: string | null
          pair_user_a: string | null
          pair_user_b: string | null
          repeat_visit_completed_at: string | null
          repeat_visit_id: string | null
        }
        Relationships: []
      }
      funnel_visit_completion: {
        Row: {
          converted: boolean | null
          pair_user_a: string | null
          pair_user_b: string | null
          visit_completed_at: string | null
          visit_id: string | null
          visit_requested_at: string | null
        }
        Relationships: []
      }
    }
    Functions: {
      accept_friend_request: {
        Args: { p_requester_user_id: string }
        Returns: {
          accepted_at: string | null
          created_at: string
          requester_id: string
          status: string
          updated_at: string
          user_a: string
          user_b: string
        }
        SetofOptions: {
          from: "*"
          to: "friendships"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      accept_visit: {
        Args: { p_idempotency_key: string; p_visit_id: string }
        Returns: {
          created_at: string
          ended_at: string | null
          ends_at: string | null
          host_user_id: string
          id: string
          request_expires_at: string
          requested_at: string
          returning_started_at: string | null
          started_at: string | null
          status: Database["public"]["Enums"]["visit_status"]
          terminal_status: Database["public"]["Enums"]["visit_status"] | null
          updated_at: string
          visitor_user_id: string
        }
        SetofOptions: {
          from: "*"
          to: "visits"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      authorize_projection_read: {
        Args: { p_owner_user_id: string }
        Returns: boolean
      }
      authorize_visit_topic: { Args: never; Returns: boolean }
      block_user: {
        Args: { p_target_user_id: string }
        Returns: {
          blocked_id: string
          blocker_id: string
          created_at: string
        }
        SetofOptions: {
          from: "*"
          to: "blocks"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      cancel_visit: {
        Args: { p_idempotency_key: string; p_visit_id: string }
        Returns: {
          created_at: string
          ended_at: string | null
          ends_at: string | null
          host_user_id: string
          id: string
          request_expires_at: string
          requested_at: string
          returning_started_at: string | null
          started_at: string | null
          status: Database["public"]["Enums"]["visit_status"]
          terminal_status: Database["public"]["Enums"]["visit_status"] | null
          updated_at: string
          visitor_user_id: string
        }
        SetofOptions: {
          from: "*"
          to: "visits"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      connector_heartbeat: {
        Args: never
        Returns: {
          connector_seen_at: string | null
          snapshot: Json
          updated_at: string
          user_id: string
        }
        SetofOptions: {
          from: "*"
          to: "pets"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      decline_visit: {
        Args: { p_idempotency_key: string; p_visit_id: string }
        Returns: {
          created_at: string
          ended_at: string | null
          ends_at: string | null
          host_user_id: string
          id: string
          request_expires_at: string
          requested_at: string
          returning_started_at: string | null
          started_at: string | null
          status: Database["public"]["Enums"]["visit_status"]
          terminal_status: Database["public"]["Enums"]["visit_status"] | null
          updated_at: string
          visitor_user_id: string
        }
        SetofOptions: {
          from: "*"
          to: "visits"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      end_visit: {
        Args: { p_idempotency_key: string; p_visit_id: string }
        Returns: {
          created_at: string
          ended_at: string | null
          ends_at: string | null
          host_user_id: string
          id: string
          request_expires_at: string
          requested_at: string
          returning_started_at: string | null
          started_at: string | null
          status: Database["public"]["Enums"]["visit_status"]
          terminal_status: Database["public"]["Enums"]["visit_status"] | null
          updated_at: string
          visitor_user_id: string
        }
        SetofOptions: {
          from: "*"
          to: "visits"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      join_waitlist: {
        Args: { p_email: string }
        Returns: {
          created_at: string
          email: string
          id: number
        }
        SetofOptions: {
          from: "*"
          to: "waitlist"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      mute_user: {
        Args: { p_muted?: boolean; p_target_user_id: string }
        Returns: {
          created_at: string
          muted: boolean
          muted_user_id: string
          owner_id: string
          updated_at: string
        }
        SetofOptions: {
          from: "*"
          to: "friend_mutes"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      recall_visit: {
        Args: { p_idempotency_key: string; p_visit_id: string }
        Returns: {
          created_at: string
          ended_at: string | null
          ends_at: string | null
          host_user_id: string
          id: string
          request_expires_at: string
          requested_at: string
          returning_started_at: string | null
          started_at: string | null
          status: Database["public"]["Enums"]["visit_status"]
          terminal_status: Database["public"]["Enums"]["visit_status"] | null
          updated_at: string
          visitor_user_id: string
        }
        SetofOptions: {
          from: "*"
          to: "visits"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      record_memory_view: {
        Args: { p_idempotency_key: string; p_memory_id: string }
        Returns: {
          created_at: string
          host_user_id: string
          id: string
          params: Json
          template_key: Database["public"]["Enums"]["memory_template_key"]
          visit_id: string
          visitor_user_id: string
        }
        SetofOptions: {
          from: "*"
          to: "shared_memories"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      redeem_invite: {
        Args: { p_code: string; p_idempotency_key: string }
        Returns: {
          id: string
          invite_code_id: string
          redeemed_at: string
          user_id: string
        }
        SetofOptions: {
          from: "*"
          to: "invite_redemptions"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      request_visit: {
        Args: { p_host_user_id: string; p_idempotency_key: string }
        Returns: {
          created_at: string
          ended_at: string | null
          ends_at: string | null
          host_user_id: string
          id: string
          request_expires_at: string
          requested_at: string
          returning_started_at: string | null
          started_at: string | null
          status: Database["public"]["Enums"]["visit_status"]
          terminal_status: Database["public"]["Enums"]["visit_status"] | null
          updated_at: string
          visitor_user_id: string
        }
        SetofOptions: {
          from: "*"
          to: "visits"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      send_friend_request: {
        Args: { p_target_user_id: string }
        Returns: {
          accepted_at: string | null
          created_at: string
          requester_id: string
          status: string
          updated_at: string
          user_a: string
          user_b: string
        }
        SetofOptions: {
          from: "*"
          to: "friendships"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      settle_shared_memory: {
        Args: { p_idempotency_key: string; p_visit_id: string }
        Returns: {
          created_at: string
          host_user_id: string
          id: string
          params: Json
          template_key: Database["public"]["Enums"]["memory_template_key"]
          visit_id: string
          visitor_user_id: string
        }
        SetofOptions: {
          from: "*"
          to: "shared_memories"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      unfriend: {
        Args: { p_other_user_id: string }
        Returns: {
          accepted_at: string | null
          created_at: string
          requester_id: string
          status: string
          updated_at: string
          user_a: string
          user_b: string
        }
        SetofOptions: {
          from: "*"
          to: "friendships"
          isOneToOne: true
          isSetofReturn: false
        }
      }
      update_projection: {
        Args: {
          p_pet_id: string
          p_skin_id: string
          p_status: Database["public"]["Enums"]["projection_status"]
        }
        Returns: {
          display_name: string
          owner_user_id: string
          pet_id: string
          skin_id: string
          status: Database["public"]["Enums"]["projection_status"]
          updated_at: string
          version: number
        }
        SetofOptions: {
          from: "*"
          to: "pet_projections"
          isOneToOne: true
          isSetofReturn: false
        }
      }
    }
    Enums: {
      memory_template_key:
        | "played_together"
        | "worked_together"
        | "celebrated_completion"
        | "shared_snack"
      projection_status:
        | "idle"
        | "working"
        | "waiting"
        | "compacting"
        | "offline"
      visit_status:
        | "requested"
        | "accepted"
        | "traveling"
        | "visiting"
        | "returning"
        | "completed"
        | "declined"
        | "cancelled"
        | "expired"
        | "recalled"
        | "blocked"
    }
    CompositeTypes: {
      [_ in never]: never
    }
  }
}

type DatabaseWithoutInternals = Omit<Database, "__InternalSupabase">

type DefaultSchema = DatabaseWithoutInternals[Extract<keyof Database, "public">]

export type Tables<
  DefaultSchemaTableNameOrOptions extends
    | keyof (DefaultSchema["Tables"] & DefaultSchema["Views"])
    | { schema: keyof DatabaseWithoutInternals },
  TableName extends DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof (DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"] &
        DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Views"])
    : never = never,
> = DefaultSchemaTableNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? (DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"] &
      DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Views"])[TableName] extends {
      Row: infer R
    }
    ? R
    : never
  : DefaultSchemaTableNameOrOptions extends keyof (DefaultSchema["Tables"] &
        DefaultSchema["Views"])
    ? (DefaultSchema["Tables"] &
        DefaultSchema["Views"])[DefaultSchemaTableNameOrOptions] extends {
        Row: infer R
      }
      ? R
      : never
    : never

export type TablesInsert<
  DefaultSchemaTableNameOrOptions extends
    | keyof DefaultSchema["Tables"]
    | { schema: keyof DatabaseWithoutInternals },
  TableName extends DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"]
    : never = never,
> = DefaultSchemaTableNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"][TableName] extends {
      Insert: infer I
    }
    ? I
    : never
  : DefaultSchemaTableNameOrOptions extends keyof DefaultSchema["Tables"]
    ? DefaultSchema["Tables"][DefaultSchemaTableNameOrOptions] extends {
        Insert: infer I
      }
      ? I
      : never
    : never

export type TablesUpdate<
  DefaultSchemaTableNameOrOptions extends
    | keyof DefaultSchema["Tables"]
    | { schema: keyof DatabaseWithoutInternals },
  TableName extends DefaultSchemaTableNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"]
    : never = never,
> = DefaultSchemaTableNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[DefaultSchemaTableNameOrOptions["schema"]]["Tables"][TableName] extends {
      Update: infer U
    }
    ? U
    : never
  : DefaultSchemaTableNameOrOptions extends keyof DefaultSchema["Tables"]
    ? DefaultSchema["Tables"][DefaultSchemaTableNameOrOptions] extends {
        Update: infer U
      }
      ? U
      : never
    : never

export type Enums<
  DefaultSchemaEnumNameOrOptions extends
    | keyof DefaultSchema["Enums"]
    | { schema: keyof DatabaseWithoutInternals },
  EnumName extends DefaultSchemaEnumNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[DefaultSchemaEnumNameOrOptions["schema"]]["Enums"]
    : never = never,
> = DefaultSchemaEnumNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[DefaultSchemaEnumNameOrOptions["schema"]]["Enums"][EnumName]
  : DefaultSchemaEnumNameOrOptions extends keyof DefaultSchema["Enums"]
    ? DefaultSchema["Enums"][DefaultSchemaEnumNameOrOptions]
    : never

export type CompositeTypes<
  PublicCompositeTypeNameOrOptions extends
    | keyof DefaultSchema["CompositeTypes"]
    | { schema: keyof DatabaseWithoutInternals },
  CompositeTypeName extends PublicCompositeTypeNameOrOptions extends {
    schema: keyof DatabaseWithoutInternals
  }
    ? keyof DatabaseWithoutInternals[PublicCompositeTypeNameOrOptions["schema"]]["CompositeTypes"]
    : never = never,
> = PublicCompositeTypeNameOrOptions extends {
  schema: keyof DatabaseWithoutInternals
}
  ? DatabaseWithoutInternals[PublicCompositeTypeNameOrOptions["schema"]]["CompositeTypes"][CompositeTypeName]
  : PublicCompositeTypeNameOrOptions extends keyof DefaultSchema["CompositeTypes"]
    ? DefaultSchema["CompositeTypes"][PublicCompositeTypeNameOrOptions]
    : never

export const Constants = {
  public: {
    Enums: {
      memory_template_key: [
        "played_together",
        "worked_together",
        "celebrated_completion",
        "shared_snack",
      ],
      projection_status: [
        "idle",
        "working",
        "waiting",
        "compacting",
        "offline",
      ],
      visit_status: [
        "requested",
        "accepted",
        "traveling",
        "visiting",
        "returning",
        "completed",
        "declined",
        "cancelled",
        "expired",
        "recalled",
        "blocked",
      ],
    },
  },
} as const
