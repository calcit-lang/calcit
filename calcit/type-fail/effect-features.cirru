
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |type-fail-effect-features
  :entries $ {} $ :default
    {}
      :description "|Effect boundary fixture: functions that touch global Refs or do IO without declaring :state / :io."
      :init-fn 'type-fail-effect-features.main/main!
      :mode :native
      :reload-fn 'type-fail-effect-features.main/main!
      :feature-policy $ {} (:dbg :error) (:io :error) (:state :error)
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'type-fail-effect-features.main
    %{} 'FileEntry
      :defs $ {}
        '*store $ %{} 'CodeEntry (:doc "|Global state.")
          :code $ quote $ defref *store 0
          :examples $ []
          :schema $ quote $ Ref Number
        'call-cb $ %{} 'CodeEntry (:doc "|Calls a callback whose Fn type declares :io.")
          :code $ quote $ defn call-cb (f) (f 1)
          :examples $ []
          :schema $ quote $ Fn
            Fn Number (-> Unit) (:features :io)
            -> Unit
        'calls-inline-io $ %{} 'CodeEntry
          :doc "|Calling an inline lambda that declares :io is a use."
          :code $ quote $ defn calls-inline-io (x)
            (fn (y) (hint-fn (Fn Number (-> Unit) (:features :io))) (println y))
              , x
          :examples $ []
          :schema $ quote $ Fn Number (-> Unit)
        'calls-log $ %{} 'CodeEntry (:doc "|Calls an :io function without declaring :io.")
          :code $ quote $ defn calls-log (x) (log-line x)
          :examples $ []
          :schema $ quote $ Fn Number (-> Unit)
        'local-acc $ %{} 'CodeEntry (:doc "|A local Ref and an assertion are not effects.")
          :code $ quote $ defn local-acc (xs) (assert= xs xs)
            let
                *acc $ ref 0
              &doseq (x xs)
                swap! *acc $ fn (a) (+ a x)
              deref *acc
          :examples $ []
          :schema $ quote $ Fn (List Number) (-> Number)
        'log-line $ %{} 'CodeEntry (:doc "|Declares :io and prints.")
          :code $ quote $ defn log-line (x) (println x)
          :examples $ []
          :schema $ quote $ Fn Number (-> Unit) (:features :io)
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (reset! *store 1) (read-store 1) (calls-log 1)
            passes-log $ [] 1 2
            local-acc $ [] 1 2
            with-handler 1
            call-cb log-line
            uses-dbg 1
            reads-env
            calls-inline-io 1
            ref-here
            shadow-ref *store
            , &unit
          :examples $ []
          :schema $ quote $ Fn (-> Unit) (:features :io :state)
        'passes-log $ %{} 'CodeEntry
          :doc "|Passes an :io function as a value without declaring :io."
          :code $ quote $ defn passes-log (xs)
            &doseq (x xs) (log-line x)
            .map xs log-line
            , &unit
          :examples $ []
          :schema $ quote $ Fn (List Number) (-> Unit)
        'read-store $ %{} 'CodeEntry (:doc "|Reads a global Ref without :state.")
          :code $ quote $ defn read-store (x)
            + x $ deref *store
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'reads-env $ %{} 'CodeEntry (:doc "|Calls a core :io wrapper without declaring :io.")
          :code $ quote $ defn reads-env () (get-env |HOME)
          :examples $ []
          :schema $ quote $ Fn
            -> $ Option String
        'ref-here $ %{} 'CodeEntry (:doc "|A Ref built at the deref site is local.")
          :code $ quote $ defn ref-here ()
            deref $ ref 0
          :examples $ []
          :schema $ quote $ Fn (-> Number)
        'shadow-ref $ %{} 'CodeEntry
          :doc "|An inner local Ref does not exempt the outer binding of the same name."
          :code $ quote $ defn shadow-ref (cell)
            let
                cell $ ref 0
              deref cell
            deref cell
          :examples $ []
          :schema $ quote $ Fn (Ref Number) (-> Number)
        'uses-dbg $ %{} 'CodeEntry
          :doc "|dbg is not :io; the entry :dbg policy controls it."
          :code $ quote $ defn uses-dbg (x) (dbg x)
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
        'with-handler $ %{} 'CodeEntry
          :doc "|Creating a lambda that declares :io is not a use."
          :code $ quote $ defn with-handler (x)
            let
                handler $ fn (y)
                  hint-fn $ Fn Number (-> Unit) (:features :io)
                  println y
              , x
          :examples $ []
          :schema $ quote $ Fn Number (-> Number)
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns type-fail-effect-features.main
