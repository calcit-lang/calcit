
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-variadic-fixed-arity
  :entries $ {}
    :default $ {}
      :description "|Fixed-arity callbacks fit variadic slots whose rest element they accept (#2612)."
      :init-fn 'type-fail-variadic-fixed-arity.main/accept-fixed-callbacks
      :mode :native
      :reload-fn 'type-fail-variadic-fixed-arity.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :nominal-extra $ {} (:description |)
      :init-fn 'type-fail-variadic-fixed-arity.main/reject-nominal-extra
      :mode :native
      :reload-fn 'type-fail-variadic-fixed-arity.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :too-few $ {} (:description |)
      :init-fn 'type-fail-variadic-fixed-arity.main/reject-too-few-params
      :mode :native
      :reload-fn 'type-fail-variadic-fixed-arity.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
    :wrong-element $ {} (:description |)
      :init-fn 'type-fail-variadic-fixed-arity.main/reject-wrong-element
      :mode :native
      :reload-fn 'type-fail-variadic-fixed-arity.main/reload!
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-variadic-fixed-arity.main
    %{} 'FileEntry
      :defs $ {}
        'accept-fixed-callbacks $ %{} 'CodeEntry
          :doc "|#2612: fixed-arity callbacks whose parameters accept the rest element fill variadic slots."
          :code $ quote $ defn accept-fixed-callbacks () (needs-number-rest add-pair) (needs-number-rest negate) (needs-number-rest zero) (needs-lead-and-rest add-pair) (needs-open-rest-callback open-pair)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'add-pair $ %{} 'CodeEntry (:doc "|Takes exactly two Number values.")
          :code $ quote $ defn add-pair (a b) (&+ a b)
          :examples $ []
          :schema $ quote $ Fn Number Number (-> Number)
        'join-pair $ %{} 'CodeEntry (:doc "|Takes exactly two String values.")
          :code $ quote $ defn join-pair (a b) (&str:concat a b)
          :examples $ []
          :schema $ quote $ Fn String String (-> String)
        'needs-lead-and-rest $ %{} 'CodeEntry
          :doc "|Slot that always passes one Number, then any number of Number values."
          :code $ quote $ defn needs-lead-and-rest (f) &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn Number & Number $ -> Number
            -> Unit
        'needs-number-rest $ %{} 'CodeEntry
          :doc "|Slot that may call its callback with any number of Number values."
          :code $ quote $ defn needs-number-rest (f) &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn & Number $ -> Number
            -> Unit
        'needs-open-rest-callback $ %{} 'CodeEntry (:doc "|Rest slot that may pass any values.")
          :code $ quote $ defn needs-open-rest-callback (f) &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn & Dynamic $ -> Unit
            -> Unit
        'needs-pair-and-rest $ %{} 'CodeEntry
          :doc "|Slot that always passes two Number values, then any number more."
          :code $ quote $ defn needs-pair-and-rest (f) &unit
          :examples $ []
          :schema $ quote $ Fn
            Fn Number Number & Number $ -> Number
            -> Unit
        'negate $ %{} 'CodeEntry (:doc "|Takes exactly one Number value.")
          :code $ quote $ defn negate (a) (&- 0 a)
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'open-pair $ %{} 'CodeEntry (:doc "|Takes exactly two values of any type.")
          :code $ quote $ defn open-pair (a b) &unit
          :examples $ []
          :schema $ quote $ Fn Dynamic Dynamic (-> Unit)
        'path-callback $ %{} 'CodeEntry (:doc "|Accepts only FsPath.")
          :code $ quote $ defn path-callback (path) &unit
          :examples $ []
          :schema $ quote $ Fn FsPath (-> Unit)
        'reject-nominal-extra $ %{} 'CodeEntry
          :doc "|A FsPath-only callback must not serve a rest slot that passes Dynamic values."
          :code $ quote $ defn reject-nominal-extra () (needs-open-rest-callback path-callback)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reject-too-few-params $ %{} 'CodeEntry
          :doc "|A one-parameter callback cannot take the two fixed values of the slot."
          :code $ quote $ defn reject-too-few-params () (needs-pair-and-rest negate)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reject-wrong-element $ %{} 'CodeEntry
          :doc "|A String callback must not fill a Number rest slot."
          :code $ quote $ defn reject-wrong-element () (needs-number-rest join-pair)
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'reload! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn reload! () &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit)
        'zero $ %{} 'CodeEntry (:doc "|Takes no arguments.")
          :code $ quote $ defn zero () 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns type-fail-variadic-fixed-arity.main
