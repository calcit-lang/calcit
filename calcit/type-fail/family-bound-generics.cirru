
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-family-bound
  :entries $ {}
    :default $ {}
      :description "|Family bounds admit named enums and structs and keep their type (#2611)."
      :init-fn 'type-fail-family-bound.main/accept-members
      :mode :native
      :reload-fn 'type-fail-family-bound.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :dynamic $ {} (:description |) (:init-fn 'type-fail-family-bound.main/reject-dynamic) (:mode :native) (:reload-fn 'type-fail-family-bound.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :enum-for-struct $ {} (:description |)
      :init-fn 'type-fail-family-bound.main/reject-enum-for-struct
      :mode :native
      :reload-fn 'type-fail-family-bound.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :family $ {} (:description |) (:init-fn 'type-fail-family-bound.main/reject-family) (:mode :native) (:reload-fn 'type-fail-family-bound.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :number $ {} (:description |) (:init-fn 'type-fail-family-bound.main/reject-number) (:mode :native) (:reload-fn 'type-fail-family-bound.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :struct-for-enum $ {} (:description |)
      :init-fn 'type-fail-family-bound.main/reject-struct-for-enum
      :mode :native
      :reload-fn 'type-fail-family-bound.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-family-bound.main
    %{} 'FileEntry
      :defs $ {}
        'Color $ %{} 'CodeEntry (:doc "|A named enum.")
          :code $ quote $ defenum Color (:red) (:green)
          :examples $ []
          :schema $ quote EnumDef
        'Point $ %{} 'CodeEntry (:doc "|A named struct.")
          :code $ quote $ defstruct Point (:x Number) (:y Number)
          :examples $ []
          :schema $ quote StructDef
        'accept-members $ %{} 'CodeEntry
          :doc "|Named members fit and the result keeps the concrete type."
          :code $ quote $ defn accept-members ()
            needs-color $ same-enum $ %:: Color :red
            needs-point $ same-struct $ %{} Point (:x 1) (:y 2)
            count-both (%:: Color :red) (%:: Color :green)
            forward $ %:: Color :green
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'count-any $ %{} 'CodeEntry (:doc "|Reflection that takes any enum.")
          :code $ quote $ defn count-any (x) (&enum:count x)
          :examples $ []
          :schema $ quote $ Fn AnyEnum (-> Number) (:open :reflect)
        'count-both $ %{} 'CodeEntry (:doc "|Reflects on two values of one enum type.")
          :code $ quote $ defn count-both (a b)
            &+ (&enum:count a) (&enum:count b)
          :examples $ []
          :schema $ quote $ Fn T T (-> Number) (:generics T)
            :where $ T AnyEnum
        'forward $ %{} 'CodeEntry
          :doc "|A bounded variable may be passed where the family is expected."
          :code $ quote $ defn forward (x) (count-any x)
          :examples $ []
          :schema $ quote $ Fn T (-> Number) (:generics T)
            :where $ T AnyEnum
        'needs-color $ %{} 'CodeEntry (:doc "|Accepts only Color.")
          :code $ quote $ defn needs-color (c) &unit
          :examples $ []
          :schema $ quote $ Fn type-fail-family-bound.main/Color (-> Unit)
        'needs-point $ %{} 'CodeEntry (:doc "|Accepts only Point.")
          :code $ quote $ defn needs-point (p) &unit
          :examples $ []
          :schema $ quote $ Fn type-fail-family-bound.main/Point (-> Unit)
        'reject-dynamic $ %{} 'CodeEntry (:doc "|Dynamic is not a named enum.")
          :code $ quote $ defn reject-dynamic (x) (same-enum x) &unit
          :examples $ []
          :schema $ quote $ Fn Dynamic (-> Unit)
        'reject-enum-for-struct $ %{} 'CodeEntry (:doc "|An enum does not fit a struct family bound.")
          :code $ quote $ defn reject-enum-for-struct ()
            same-struct $ %:: Color :red
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reject-family $ %{} 'CodeEntry (:doc "|The family itself is not a named enum.")
          :code $ quote $ defn reject-family (x) (same-enum x) &unit
          :examples $ []
          :schema $ quote $ Fn AnyEnum (-> Unit) (:open :reflect)
        'reject-number $ %{} 'CodeEntry (:doc "|A Number is not an enum.")
          :code $ quote $ defn reject-number () (same-enum 1) &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reject-struct-for-enum $ %{} 'CodeEntry (:doc "|A struct does not fit an enum family bound.")
          :code $ quote $ defn reject-struct-for-enum ()
            same-enum $ %{} Point (:x 1) (:y 2)
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'same-enum $ %{} 'CodeEntry
          :doc "|Returns its enum argument with the caller's type."
          :code $ quote $ defn same-enum (x) x
          :examples $ []
          :schema $ quote $ Fn T (-> T) (:generics T)
            :where $ T AnyEnum
        'same-struct $ %{} 'CodeEntry
          :doc "|Returns its struct argument with the caller's type."
          :code $ quote $ defn same-struct (x) x
          :examples $ []
          :schema $ quote $ Fn T (-> T) (:generics T)
            :where $ T AnyStruct
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns type-fail-family-bound.main
