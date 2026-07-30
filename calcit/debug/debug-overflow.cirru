
{} (:about "|Machine-generated snapshot. Do not edit directly — changes will be overwritten. Use `cr query` to inspect and `cr edit`/`cr tree` to modify. Run `cr docs agents --full` first. Manual edits must follow format and schema conventions, then run `cr edit format`.") (:package |debug-overflow) (:version |0.0.0)
  :entries $ {}
    :default $ {} (:description |) (:init-fn |debug-overflow.main/main!) (:mode :native) (:reload-fn |debug-overflow.main/reload!)
      :modules $ []
      :type-slots $ {}
  :files $ {}
    |debug-overflow.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defn main! () (println |TODO) (; rec 1 2 3 4 5 6 7 8 9)
              println $ my-cond
                  &> 2 1
                  , 1
                (&> 3 2) 2
                true 0
          :examples $ []
        |my-cond $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defmacro my-cond (pair & else)
              &let
                expr $ nth pair 0
                &let
                  branch $ nth pair 1
                  quasiquote $ if ~expr ~branch
                    ~ $ if (empty? else) (:: :unit)
                      quasiquote $ my-cond
                        ~ $ nth else 0
                        ~@ $ rest else
          :examples $ []
        |rec $ %{} :CodeEntry (:doc |) (:schema :dynamic)
          :code $ quote
            defmacro rec (x0 & xs)
              quasiquote $ if (&> ~x0 10) "|Too large"
                if
                  ~ $ empty? xs
                  , ~x0 $ &+ ~x0
                    rec $ ~@ xs
          :examples $ []
      :ns $ %{} :NsEntry (:doc |)
        :code $ quote
          ns debug-overflow.main $ :require
            [] util.core :refer $ [] log-title inside-eval:
