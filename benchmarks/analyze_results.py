#!/usr/bin/env python3
"""
Benchmark result analyzer for Palladium
Compares performance with C and generates reports
"""

import json
import sys
from datetime import datetime
from pathlib import Path

class BenchmarkAnalyzer:
    def __init__(self):
        self.results = {
            'timestamp': datetime.now().isoformat(),
            'benchmarks': {}
        }
    
    def add_result(self, name, c_time, pd_c_time, pd_llvm_time=None):
        """Add a benchmark result"""
        self.results['benchmarks'][name] = {
            'c_time': c_time,
            'palladium_c_backend': pd_c_time,
            'palladium_llvm_backend': pd_llvm_time,
            'c_to_pd_c_ratio': pd_c_time / c_time if c_time > 0 else None,
            'c_to_pd_llvm_ratio': pd_llvm_time / c_time if pd_llvm_time and c_time > 0 else None
        }
    
    def generate_report(self):
        """Generate a markdown report"""
        report = f"""# Palladium Benchmark Results
*Generated: {self.results['timestamp']}*

## Summary

| Benchmark | C Time | Palladium (C) | Palladium (LLVM) | C→Pd(C) Ratio | C→Pd(LLVM) Ratio |
|-----------|--------|---------------|-------------------|---------------|-------------------|
"""
        
        for name, data in self.results['benchmarks'].items():
            c_time = f"{data['c_time']:.3f}s" if data['c_time'] else "N/A"
            pd_c_time = f"{data['palladium_c_backend']:.3f}s" if data['palladium_c_backend'] else "N/A"
            pd_llvm_time = f"{data['palladium_llvm_backend']:.3f}s" if data['palladium_llvm_backend'] else "N/A"
            
            c_pd_c_ratio = f"{data['c_to_pd_c_ratio']:.2f}x" if data['c_to_pd_c_ratio'] else "N/A"
            c_pd_llvm_ratio = f"{data['c_to_pd_llvm_ratio']:.2f}x" if data['c_to_pd_llvm_ratio'] else "N/A"
            
            # Add emoji indicators
            if data['c_to_pd_c_ratio']:
                if data['c_to_pd_c_ratio'] <= 1.1:
                    c_pd_c_ratio += " ✅"
                elif data['c_to_pd_c_ratio'] <= 1.5:
                    c_pd_c_ratio += " ⚠️"
                else:
                    c_pd_c_ratio += " ❌"
            
            report += f"| {name} | {c_time} | {pd_c_time} | {pd_llvm_time} | {c_pd_c_ratio} | {c_pd_llvm_ratio} |\n"
        
        report += """
## Analysis

### Performance Goals
- ✅ Within 10% of C (ratio ≤ 1.1)
- ⚠️ Within 50% of C (ratio ≤ 1.5)
- ❌ More than 50% slower than C (ratio > 1.5)

### Observations
"""
        
        # Add specific observations
        total_benchmarks = len(self.results['benchmarks'])
        within_10_percent = sum(1 for b in self.results['benchmarks'].values() 
                               if b['c_to_pd_c_ratio'] and b['c_to_pd_c_ratio'] <= 1.1)
        
        report += f"- {within_10_percent}/{total_benchmarks} benchmarks within 10% of C performance\n"
        
        # Find best and worst performers
        if self.results['benchmarks']:
            ratios = [(name, b['c_to_pd_c_ratio']) for name, b in self.results['benchmarks'].items() 
                     if b['c_to_pd_c_ratio']]
            if ratios:
                best = min(ratios, key=lambda x: x[1])
                worst = max(ratios, key=lambda x: x[1])
                report += f"- Best performer: {best[0]} ({best[1]:.2f}x of C)\n"
                report += f"- Worst performer: {worst[0]} ({worst[1]:.2f}x of C)\n"
        
        report += """
### Next Steps
1. Profile worst-performing benchmarks
2. Optimize hot paths in code generation
3. Implement LLVM optimization passes
4. Add more realistic benchmarks
"""
        
        return report
    
    def save_results(self, output_dir="results"):
        """Save results to JSON and markdown"""
        output_path = Path(output_dir)
        output_path.mkdir(exist_ok=True)
        
        # Save JSON
        timestamp = datetime.now().strftime("%Y%m%d_%H%M%S")
        json_file = output_path / f"benchmark_results_{timestamp}.json"
        with open(json_file, 'w') as f:
            json.dump(self.results, f, indent=2)
        
        # Save markdown report
        md_file = output_path / f"benchmark_report_{timestamp}.md"
        with open(md_file, 'w') as f:
            f.write(self.generate_report())
        
        # Update latest symlink
        latest_md = output_path / "latest_report.md"
        if latest_md.exists():
            latest_md.unlink()
        latest_md.symlink_to(md_file.name)
        
        print(f"Results saved to {json_file}")
        print(f"Report saved to {md_file}")


def main():
    """Example usage"""
    analyzer = BenchmarkAnalyzer()
    
    # Example data (would be parsed from actual benchmark output)
    analyzer.add_result("fibonacci", 1.234, 1.456, 1.289)
    analyzer.add_result("matrix_multiply", 2.345, 2.678, 2.456)
    analyzer.add_result("string_concat", 0.123, 0.234, None)
    analyzer.add_result("bubble_sort", 3.456, 4.567, 3.789)
    
    print(analyzer.generate_report())
    analyzer.save_results()


if __name__ == "__main__":
    main()