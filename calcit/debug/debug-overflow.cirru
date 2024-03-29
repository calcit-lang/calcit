
{} (:package |debug-overflow)
  :configs $ {} (:init-fn |debug-overflow.main/main!) (:reload-fn |debug-overflow.main/reload!)
    :modules $ []
  :files $ {}
    |debug-overflow.main $ %{} :FileEntry
      :defs $ {}
        |main! $ %{} :CodeEntry (:doc |)
          :code $ quote
            defn main! () (println |TODO) (; rec 1 2 3 4 5 6 7 8 9)
              println $ my-cond
                  &> 2 1
                  , 1
                (&> 3 2) 2
                true 0
        |my-cond $ %{} :CodeEntry (:doc |)
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
        |rec $ %{} :CodeEntry (:doc |)
          :code $ quote
            defmacro rec (x0 & xs)
              quasiquote $ if (&> ~x0 10) "|Too large"
                if
                  ~ $ empty? xs
                  , ~x0 $ &+ ~x0
                    rec $ ~@ xs
      :ns $ %{} :CodeEntry (:doc |)
        :code $ quote
          ns debug-overflow.main $ :require
            [] util.core :refer $ [] log-title inside-eval:
