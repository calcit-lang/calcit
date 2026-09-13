
{}
  :about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `calcit query` to inspect and `calcit edit`/`calcit tree` to modify. Run `calcit docs agents --contract` before mutations; use `--full` for first orientation or changed contract digest. Manual edits must follow format and schema conventions, then run `calcit edit format`."
  :package |debug-overflow
  :entries $ {} $ :default
    {} (:description |) (:init-fn 'debug-overflow.main/main!) (:mode :native) (:reload-fn 'debug-overflow.main/reload!)
      :feature-policy $ {}
      :modules $ []
      :type-slots $ {}
  :files $ {} $ 'debug-overflow.main
    %{} 'FileEntry
      :defs $ {}
        'main! $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defn main! () (println |TODO)
            ; rec 1 2 3 4 5 6 7 8 9
            println $ my-cond
                &> 2 1
                , 1
              (&> 3 2) 2
              true 0
          :examples $ []
          :schema $ :: 'Dynamic
        'my-cond $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro my-cond (pair & else)
            &let
              expr $ nth pair 0
              &let
                branch $ nth pair 1
                quasiquote $ if ~expr ~branch $ ~
                  if (empty? else) (:: :unit)
                    quasiquote $ my-cond
                      ~ $ nth else 0
                      ~@ $ rest else
          :examples $ []
          :schema $ :: 'Macro $ {} (:rest 'Syntax)
            :capabilities $ #{}
            :expansion $ :: 'Expr 'Dynamic
            :required $ [] 'SyntaxList
        'rec $ %{} 'CodeEntry (:doc |)
          :code $ quote $ defmacro rec (x0 & xs)
            quasiquote $ if (&> ~x0 10) "|Too large" $ if
              ~ $ empty? xs
              , ~x0
                &+ ~x0 $ rec $ ~@ xs
          :examples $ []
          :schema $ :: 'Macro $ {} (:rest 'Syntax)
            :capabilities $ #{}
            :expansion $ :: 'Expr 'Dynamic
            :required $ [] $ :: 'Expr 'Dynamic
      :ns $ %{} 'NsEntry (:doc |)
        :code $ quote $ ns debug-overflow.main
          :require $ [] util.core :refer $ [] log-title inside-eval:
